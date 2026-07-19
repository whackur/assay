mod entry;
mod error;
mod history;
mod limits;
mod object_id;
mod path;
mod provenance;
mod snapshot;

pub use entry::{EntryMode, ObjectIssue, ObjectKind, ObjectMetadata, TrackedEntry};
pub use error::{CollectionError, CollectionErrorKind, CollectionStage};
pub use history::{HistoryAvailability, HistoryIssue, ParentDelta, ParentDeltaIssue};
pub use limits::CollectionLimits;
pub use object_id::{GitObjectFormat, GitObjectId};
pub use path::RepositoryPath;
pub use provenance::GitProvenance;
pub use snapshot::{
    RepositorySnapshot, RepositorySnapshotPort, ResolvedLocalRepository, SnapshotRequest,
};
