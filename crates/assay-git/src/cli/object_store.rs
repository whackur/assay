use std::{
    ffi::OsStr,
    fs,
    path::{Path, PathBuf},
};

use crate::{CollectionError, CollectionErrorKind, CollectionStage, GitObjectFormat};

use super::adapter::GitCliAdapter;
use super::error::{external_object_store, repository_redirect};
use super::parse::{parse_boolean, single_line};

impl GitCliAdapter {
    pub(crate) fn validate_object_store(
        &self,
        repository: &Path,
        topology: &crate::topology::RepositoryTopology,
    ) -> Result<(), CollectionError> {
        let git_directory = self.reported_path(
            repository,
            &[
                OsStr::new("rev-parse"),
                OsStr::new("--path-format=absolute"),
                OsStr::new("--absolute-git-dir"),
            ],
        )?;
        let common_directory = self.reported_path(
            repository,
            &[
                OsStr::new("rev-parse"),
                OsStr::new("--path-format=absolute"),
                OsStr::new("--git-common-dir"),
            ],
        )?;
        if git_directory != topology.git_directory()
            || common_directory != topology.common_directory()
        {
            return Err(repository_redirect());
        }
        let bare_output = self.runner.run(
            Some(repository),
            CollectionStage::ValidateObjectStore,
            &[OsStr::new("rev-parse"), OsStr::new("--is-bare-repository")],
            16,
        )?;
        let reported_bare = parse_boolean(&bare_output, CollectionStage::ValidateObjectStore)?;
        if reported_bare != (topology.kind() == crate::topology::RepositoryKind::Bare) {
            return Err(repository_redirect());
        }
        if topology.kind() != crate::topology::RepositoryKind::Bare {
            let top_level = self.reported_path(
                repository,
                &[
                    OsStr::new("rev-parse"),
                    OsStr::new("--path-format=absolute"),
                    OsStr::new("--show-toplevel"),
                ],
            )?;
            if top_level != topology.repository() {
                return Err(repository_redirect());
            }
        }

        let objects = topology.common_directory().join("objects");
        let metadata = fs::symlink_metadata(&objects).map_err(|_| {
            CollectionError::new(
                CollectionStage::ValidateObjectStore,
                CollectionErrorKind::ExternalObjectStore,
            )
        })?;
        if !metadata.is_dir() || metadata.file_type().is_symlink() {
            return Err(external_object_store());
        }
        for alternates in [
            objects.join("info/alternates"),
            objects.join("info/http-alternates"),
        ] {
            match fs::symlink_metadata(alternates) {
                Ok(_) => return Err(external_object_store()),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(_) => {
                    return Err(CollectionError::new(
                        CollectionStage::ValidateObjectStore,
                        CollectionErrorKind::Io,
                    ));
                }
            }
        }
        reject_object_store_symlinks(&objects, self.limits.max_object_store_entries)
    }

    pub(crate) fn reported_path(
        &self,
        repository: &Path,
        arguments: &[&OsStr],
    ) -> Result<PathBuf, CollectionError> {
        let output = self.runner.run(
            Some(repository),
            CollectionStage::ValidateObjectStore,
            arguments,
            16 * 1024,
        )?;
        let path =
            path_from_git_output(single_line(&output, CollectionStage::ValidateObjectStore)?)?;
        if !path.is_absolute() {
            return Err(repository_redirect());
        }
        fs::canonicalize(path).map_err(|_| repository_redirect())
    }

    pub(crate) fn object_format(
        &self,
        repository: &Path,
    ) -> Result<GitObjectFormat, CollectionError> {
        let output = self.runner.run(
            Some(repository),
            CollectionStage::ValidateObjectStore,
            &[
                OsStr::new("rev-parse"),
                OsStr::new("--show-object-format=storage"),
            ],
            32,
        )?;
        match single_line(&output, CollectionStage::ValidateObjectStore)? {
            b"sha1" => Ok(GitObjectFormat::Sha1),
            b"sha256" => Ok(GitObjectFormat::Sha256),
            _ => Err(CollectionError::new(
                CollectionStage::ValidateObjectStore,
                CollectionErrorKind::MalformedOutput,
            )),
        }
    }

    pub(crate) fn is_shallow(&self, repository: &Path) -> Result<bool, CollectionError> {
        let output = self.runner.run(
            Some(repository),
            CollectionStage::ReadHistory,
            &[
                OsStr::new("rev-parse"),
                OsStr::new("--is-shallow-repository"),
            ],
            16,
        )?;
        parse_boolean(&output, CollectionStage::ReadHistory)
    }
}

pub(crate) fn reject_object_store_symlinks(
    objects: &Path,
    maximum_entries: usize,
) -> Result<(), CollectionError> {
    let mut pending = vec![objects.to_path_buf()];
    let mut inspected = 0_usize;
    while let Some(directory) = pending.pop() {
        let children = fs::read_dir(directory).map_err(|_| {
            CollectionError::new(
                CollectionStage::ValidateObjectStore,
                CollectionErrorKind::Io,
            )
        })?;
        for child in children {
            let child = child.map_err(|_| {
                CollectionError::new(
                    CollectionStage::ValidateObjectStore,
                    CollectionErrorKind::Io,
                )
            })?;
            inspected = inspected.checked_add(1).ok_or_else(|| {
                CollectionError::new(
                    CollectionStage::ValidateObjectStore,
                    CollectionErrorKind::RecordLimit,
                )
            })?;
            if inspected > maximum_entries {
                return Err(CollectionError::new(
                    CollectionStage::ValidateObjectStore,
                    CollectionErrorKind::RecordLimit,
                ));
            }
            let metadata = fs::symlink_metadata(child.path()).map_err(|_| {
                CollectionError::new(
                    CollectionStage::ValidateObjectStore,
                    CollectionErrorKind::Io,
                )
            })?;
            if metadata.file_type().is_symlink() {
                return Err(external_object_store());
            }
            if metadata.is_dir() {
                pending.push(child.path());
            }
        }
    }
    Ok(())
}

#[cfg(unix)]
pub(crate) fn path_from_git_output(bytes: &[u8]) -> Result<PathBuf, CollectionError> {
    use std::os::unix::ffi::OsStringExt;

    Ok(PathBuf::from(std::ffi::OsString::from_vec(bytes.to_vec())))
}

#[cfg(not(unix))]
pub(crate) fn path_from_git_output(bytes: &[u8]) -> Result<PathBuf, CollectionError> {
    let value = std::str::from_utf8(bytes).map_err(|_| {
        CollectionError::new(
            CollectionStage::ValidateObjectStore,
            CollectionErrorKind::MalformedOutput,
        )
    })?;
    Ok(PathBuf::from(value))
}
