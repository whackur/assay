use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::enums::{HostedAdmission, HostedRequestState};
use super::{HOSTED_API_CONTRACT, HOSTED_API_SCHEMA_VERSION};

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HostedSubmissionRequest {
    pub repository: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct HostedContractEnvelope<T> {
    contract: &'static str,
    schema_version: &'static str,
    data: T,
}

impl<T> HostedContractEnvelope<T> {
    pub const fn new(data: T) -> Self {
        Self {
            contract: HOSTED_API_CONTRACT,
            schema_version: HOSTED_API_SCHEMA_VERSION,
            data,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct HostedErrorResponse {
    contract: &'static str,
    schema_version: &'static str,
    error: HostedError,
}

impl HostedErrorResponse {
    pub const fn new(code: &'static str) -> Self {
        Self {
            contract: HOSTED_API_CONTRACT,
            schema_version: HOSTED_API_SCHEMA_VERSION,
            error: HostedError { code },
        }
    }
}

#[derive(Clone, Debug, Serialize)]
struct HostedError {
    code: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub struct HostedSubmission {
    request_id: String,
    owner: String,
    repository: String,
    canonical_url: String,
    state: HostedRequestState,
    admission: HostedAdmission,
    retry_after_seconds: Option<i64>,
}

impl HostedSubmission {
    pub fn new(
        request_id: Uuid,
        owner: String,
        repository: String,
        state: HostedRequestState,
        admission: HostedAdmission,
        retry_after_seconds: Option<i64>,
    ) -> Self {
        let canonical_url = format!("https://github.com/{owner}/{repository}");
        Self {
            request_id: request_id.to_string(),
            owner,
            repository,
            canonical_url,
            state,
            admission,
            retry_after_seconds,
        }
    }

    pub const fn admission(&self) -> HostedAdmission {
        self.admission
    }

    pub const fn state(&self) -> HostedRequestState {
        self.state
    }

    pub const fn retry_after_seconds(&self) -> Option<i64> {
        self.retry_after_seconds
    }
}
