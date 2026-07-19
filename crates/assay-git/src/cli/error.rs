use crate::{
    CollectionError, CollectionErrorKind, CollectionStage, ObjectIssue, ParentDelta,
    ParentDeltaIssue,
};

pub(crate) fn incompatible_git() -> CollectionError {
    CollectionError::new(
        CollectionStage::ProbeCapabilities,
        CollectionErrorKind::IncompatibleGit,
    )
}

pub(crate) fn malformed_parent_delta() -> CollectionError {
    CollectionError::new(
        CollectionStage::ReadParentDelta,
        CollectionErrorKind::MalformedOutput,
    )
}

pub(crate) fn external_object_store() -> CollectionError {
    CollectionError::new(
        CollectionStage::ValidateObjectStore,
        CollectionErrorKind::ExternalObjectStore,
    )
}

pub(crate) fn repository_redirect() -> CollectionError {
    CollectionError::new(
        CollectionStage::ValidateObjectStore,
        CollectionErrorKind::RepositoryRedirect,
    )
}

pub(crate) fn object_issue(kind: CollectionErrorKind) -> ObjectIssue {
    match kind {
        CollectionErrorKind::Timeout => ObjectIssue::Timeout,
        CollectionErrorKind::OutputLimit => ObjectIssue::OutputLimit,
        CollectionErrorKind::MalformedOutput => ObjectIssue::MalformedMetadata,
        _ => ObjectIssue::MissingOrUnreadable,
    }
}

pub(crate) fn unavailable_parent_delta(issue: ParentDeltaIssue) -> ParentDelta {
    ParentDelta::new(assay_domain::EvidenceStatus::Unavailable, 0, 0, Some(issue))
}
