//! Static commit and file specifications used by [`RepositoryScenario`].

pub struct CommitSpec {
    pub(crate) message: &'static str,
    pub(crate) files: Vec<FileSpec>,
    pub(crate) removals: Vec<&'static str>,
}

impl CommitSpec {
    pub(crate) fn new(
        message: &'static str,
        files: &[FileSpec],
        removals: &[&'static str],
    ) -> Self {
        Self {
            message,
            files: files.to_vec(),
            removals: removals.to_vec(),
        }
    }
}

#[derive(Clone, Copy)]
pub struct FileSpec {
    pub(crate) path: &'static str,
    pub(crate) contents: &'static [u8],
}

impl FileSpec {
    pub(crate) const fn new(path: &'static str, contents: &'static [u8]) -> Self {
        Self { path, contents }
    }
}
