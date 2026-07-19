//! Hosted source-processing machine contracts and projection policy.
//!
//! These contracts report collection and evaluation workflow state. They do
//! not represent project-catalog publication, a provider judgment, or a score.

mod enums;
mod status;
mod submission;

pub use enums::{
    HostedAdmission, HostedContractValueError, HostedEvaluationStatus, HostedJobStage,
    HostedJobState, HostedRequestState, HostedScoreStatus,
};
pub use status::{
    HostedProjectStatus, HostedProjectStatusRecord, HostedRecentSourceStatus,
    HostedRecentSourceStatusRecord,
};
pub use submission::{
    HostedContractEnvelope, HostedErrorResponse, HostedSubmission, HostedSubmissionRequest,
};

pub const HOSTED_API_CONTRACT: &str = "assay-hosted-api";
pub const HOSTED_API_SCHEMA_VERSION: &str = "1.0.0";
