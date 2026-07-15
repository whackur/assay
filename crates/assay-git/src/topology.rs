use std::{
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
};

use crate::{CollectionError, CollectionErrorKind, CollectionStage};

const MAX_POINTER_BYTES: u64 = 16 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RepositoryKind {
    WorkingTree,
    LinkedWorktree,
    Bare,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RepositoryTopology {
    repository: PathBuf,
    git_directory: PathBuf,
    common_directory: PathBuf,
    kind: RepositoryKind,
}

impl RepositoryTopology {
    pub(crate) fn inspect(submitted: &Path) -> Result<Self, CollectionError> {
        let submitted_metadata = fs::symlink_metadata(submitted).map_err(|_| topology_error())?;
        if !submitted_metadata.is_dir() || submitted_metadata.file_type().is_symlink() {
            return Err(topology_error());
        }
        let repository = fs::canonicalize(submitted).map_err(|_| topology_error())?;
        let dot_git = submitted.join(".git");
        match fs::symlink_metadata(&dot_git) {
            Ok(metadata) if metadata.file_type().is_symlink() => Err(topology_error()),
            Ok(metadata) if metadata.is_dir() => {
                let git_directory = fs::canonicalize(&dot_git).map_err(|_| topology_error())?;
                Ok(Self {
                    repository,
                    common_directory: git_directory.clone(),
                    git_directory,
                    kind: RepositoryKind::WorkingTree,
                })
            }
            Ok(metadata) if metadata.is_file() => Self::linked_worktree(repository, &dot_git),
            Ok(_) => Err(topology_error()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Self::bare(repository),
            Err(_) => Err(topology_error()),
        }
    }

    fn bare(repository: PathBuf) -> Result<Self, CollectionError> {
        for relative in ["HEAD", "config"] {
            let metadata =
                fs::symlink_metadata(repository.join(relative)).map_err(|_| topology_error())?;
            if !metadata.is_file() || metadata.file_type().is_symlink() {
                return Err(topology_error());
            }
        }
        for relative in ["objects", "refs"] {
            let metadata =
                fs::symlink_metadata(repository.join(relative)).map_err(|_| topology_error())?;
            if !metadata.is_dir() || metadata.file_type().is_symlink() {
                return Err(topology_error());
            }
        }
        Ok(Self {
            git_directory: repository.clone(),
            common_directory: repository.clone(),
            repository,
            kind: RepositoryKind::Bare,
        })
    }

    fn linked_worktree(repository: PathBuf, dot_git: &Path) -> Result<Self, CollectionError> {
        let pointer = read_regular_file(dot_git)?;
        let target = pointer
            .strip_prefix(b"gitdir: ")
            .ok_or_else(topology_error)?;
        let git_directory = canonical_pointer(
            dot_git.parent().ok_or_else(topology_error)?,
            one_line(target)?,
        )?;
        let git_metadata = fs::symlink_metadata(&git_directory).map_err(|_| topology_error())?;
        if !git_metadata.is_dir() || git_metadata.file_type().is_symlink() {
            return Err(topology_error());
        }

        let common_pointer = read_regular_file(&git_directory.join("commondir"))?;
        let common_directory = canonical_pointer(&git_directory, one_line(&common_pointer)?)?;
        let common_metadata =
            fs::symlink_metadata(&common_directory).map_err(|_| topology_error())?;
        if !common_metadata.is_dir() || common_metadata.file_type().is_symlink() {
            return Err(topology_error());
        }
        let worktrees = git_directory.parent().ok_or_else(topology_error)?;
        if worktrees.file_name().and_then(|name| name.to_str()) != Some("worktrees")
            || worktrees.parent() != Some(common_directory.as_path())
        {
            return Err(topology_error());
        }

        let backlink = read_regular_file(&git_directory.join("gitdir"))?;
        let backlink = canonical_pointer(&git_directory, one_line(&backlink)?)?;
        let submitted_dot_git = fs::canonicalize(dot_git).map_err(|_| topology_error())?;
        if backlink != submitted_dot_git {
            return Err(topology_error());
        }

        Ok(Self {
            repository,
            git_directory,
            common_directory,
            kind: RepositoryKind::LinkedWorktree,
        })
    }

    pub(crate) fn repository(&self) -> &Path {
        &self.repository
    }

    pub(crate) fn git_directory(&self) -> &Path {
        &self.git_directory
    }

    pub(crate) fn common_directory(&self) -> &Path {
        &self.common_directory
    }

    pub(crate) const fn kind(&self) -> RepositoryKind {
        self.kind
    }
}

fn read_regular_file(path: &Path) -> Result<Vec<u8>, CollectionError> {
    let metadata = fs::symlink_metadata(path).map_err(|_| topology_error())?;
    if !metadata.is_file()
        || metadata.file_type().is_symlink()
        || metadata.len() == 0
        || metadata.len() > MAX_POINTER_BYTES
    {
        return Err(topology_error());
    }
    let bytes = fs::read(path).map_err(|_| topology_error())?;
    if u64::try_from(bytes.len()).ok() != Some(metadata.len()) {
        return Err(topology_error());
    }
    Ok(bytes)
}

fn one_line(bytes: &[u8]) -> Result<&[u8], CollectionError> {
    let value = bytes.strip_suffix(b"\n").unwrap_or(bytes);
    if value.is_empty() || value.contains(&b'\n') || value.contains(&b'\r') || value.contains(&0) {
        return Err(topology_error());
    }
    Ok(value)
}

fn canonical_pointer(base: &Path, bytes: &[u8]) -> Result<PathBuf, CollectionError> {
    let path = PathBuf::from(os_string(bytes)?);
    let joined = if path.is_absolute() {
        path
    } else {
        base.join(path)
    };
    let metadata = fs::symlink_metadata(&joined).map_err(|_| topology_error())?;
    if metadata.file_type().is_symlink() {
        return Err(topology_error());
    }
    fs::canonicalize(joined).map_err(|_| topology_error())
}

#[cfg(unix)]
fn os_string(bytes: &[u8]) -> Result<OsString, CollectionError> {
    use std::os::unix::ffi::OsStringExt;

    Ok(OsString::from_vec(bytes.to_vec()))
}

#[cfg(not(unix))]
fn os_string(bytes: &[u8]) -> Result<OsString, CollectionError> {
    let value = std::str::from_utf8(bytes).map_err(|_| topology_error())?;
    Ok(OsString::from(value))
}

fn topology_error() -> CollectionError {
    CollectionError::new(
        CollectionStage::ValidateObjectStore,
        CollectionErrorKind::RepositoryRedirect,
    )
}
