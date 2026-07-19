mod collector;
mod error;
mod reader;
mod source;

pub use collector::GitHubCollector;
pub use error::{CollectionError, CollectionErrorKind, CollectionStage};
pub use source::{GitHubRepositoryMetadata, ResolvedGitHubSource, RevisionSelector};

pub(crate) use reader::{LimitedReader, content_length_exceeds, is_response_limit};
