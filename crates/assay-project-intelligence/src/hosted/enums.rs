use serde::Serialize;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HostedRequestState {
    Queued,
    Collecting,
    Partial,
    Complete,
    Unavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HostedAdmission {
    Admitted,
    JoinedActive,
    Cooldown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HostedJobStage {
    Canonicalizing,
    Collecting,
    Evaluating,
    Publishing,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HostedJobState {
    Queued,
    Running,
    Partial,
    Complete,
    Unavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HostedEvaluationStatus {
    ValidatedUnpublished,
    Partial,
    Unavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HostedScoreStatus {
    Pending,
    Complete,
    Partial,
    Insufficient,
    Unavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HostedContractValueError;

macro_rules! storage_enum {
    ($type:ty, { $($value:literal => $variant:path),+ $(,)? }) => {
        impl TryFrom<&str> for $type {
            type Error = HostedContractValueError;

            fn try_from(value: &str) -> Result<Self, Self::Error> {
                match value {
                    $($value => Ok($variant),)+
                    _ => Err(HostedContractValueError),
                }
            }
        }
    };
}

storage_enum!(HostedRequestState, {
    "queued" => HostedRequestState::Queued,
    "collecting" => HostedRequestState::Collecting,
    "running" => HostedRequestState::Collecting,
    "partial" => HostedRequestState::Partial,
    "complete" => HostedRequestState::Complete,
    "unavailable" => HostedRequestState::Unavailable,
});
storage_enum!(HostedAdmission, {
    "admitted" => HostedAdmission::Admitted,
    "joined_active" => HostedAdmission::JoinedActive,
    "cooldown" => HostedAdmission::Cooldown,
});
storage_enum!(HostedJobStage, {
    "canonicalizing" => HostedJobStage::Canonicalizing,
    "collecting" => HostedJobStage::Collecting,
    "evaluating" => HostedJobStage::Evaluating,
    "publishing" => HostedJobStage::Publishing,
});
storage_enum!(HostedJobState, {
    "queued" => HostedJobState::Queued,
    "running" => HostedJobState::Running,
    "partial" => HostedJobState::Partial,
    "complete" => HostedJobState::Complete,
    "unavailable" => HostedJobState::Unavailable,
});
storage_enum!(HostedEvaluationStatus, {
    "validated_unpublished" => HostedEvaluationStatus::ValidatedUnpublished,
    "partial" => HostedEvaluationStatus::Partial,
    "unavailable" => HostedEvaluationStatus::Unavailable,
});
storage_enum!(HostedScoreStatus, {
    "pending" => HostedScoreStatus::Pending,
    "complete" => HostedScoreStatus::Complete,
    "partial" => HostedScoreStatus::Partial,
    "insufficient" => HostedScoreStatus::Insufficient,
    "unavailable" => HostedScoreStatus::Unavailable,
});
