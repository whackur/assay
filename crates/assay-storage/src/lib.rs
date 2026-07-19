//! PostgreSQL persistence adapter for the hosted Assay runtime.
//!
//! Raw provider payloads and credentials never cross this boundary. Immutable
//! facts are append-only, admission is serialized, and every worker mutation
//! is fenced by the current job generation and lease token.

mod admission;
mod admission_bucket;
mod ai_analysis;
mod approval;
mod error;
mod evaluation;
mod failure;
mod failure_circuit;
mod job;
mod rows;
mod source;
mod status;
mod storage;
#[cfg(test)]
mod tests;
mod types;
mod util;
mod workflow;

pub use ai_analysis::{REVIEW_QUEUE_LIMIT, ReviewQueueItem, ReviewQueueProvenance};
pub use approval::PublicationApproval;
pub use error::{AdmissionError, StorageError};
pub use storage::Storage;
pub use types::{
    ClaimedJob, EvaluationAttempt, FailureSettlement, GitHubCollection, PublicAdmissionLimits,
    StoredEvaluationInput, StoredSource,
};
