use assay_domain::EvidenceStatus;
use assay_git::{EntryMode, ObjectKind};

use crate::evidence::types::{
    ClassificationAvailabilityReason, ClassificationCategoryRecord,
    ClassificationEvidenceKindRecord, ClassificationTagRecord, RawEvidenceIssue,
};

pub(crate) const fn entry_mode_code(value: EntryMode) -> &'static str {
    match value {
        EntryMode::Regular => "regular",
        EntryMode::Executable => "executable",
        EntryMode::SymbolicLink => "symbolic_link",
        EntryMode::Gitlink => "gitlink",
    }
}

pub(crate) const fn object_kind_code(value: ObjectKind) -> &'static str {
    match value {
        ObjectKind::Blob => "blob",
        ObjectKind::Commit => "commit",
    }
}

pub(crate) const fn raw_issue_code(value: RawEvidenceIssue) -> &'static str {
    match value {
        RawEvidenceIssue::GitlinkContent => "gitlink_content",
        RawEvidenceIssue::SizeLimit => "size_limit",
        RawEvidenceIssue::MissingOrUnreadable => "missing_or_unreadable",
        RawEvidenceIssue::Timeout => "timeout",
        RawEvidenceIssue::OutputLimit => "output_limit",
        RawEvidenceIssue::MalformedMetadata => "malformed_metadata",
        RawEvidenceIssue::HistoryDepthLimit => "history_depth_limit",
        RawEvidenceIssue::ShallowRepository => "shallow_repository",
        RawEvidenceIssue::ProcessFailure => "process_failure",
        RawEvidenceIssue::MalformedOutput => "malformed_output",
        RawEvidenceIssue::RenameCandidateLimit => "rename_candidate_limit",
    }
}

pub(crate) const fn classification_reason_code(
    value: ClassificationAvailabilityReason,
) -> &'static str {
    match value {
        ClassificationAvailabilityReason::AttributesUnavailable => "attributes_unavailable",
        ClassificationAvailabilityReason::MissingClassification => "missing_classification",
        ClassificationAvailabilityReason::NonPortablePath => "non_portable_path",
    }
}

pub(crate) const fn classification_category_code(
    value: ClassificationCategoryRecord,
) -> &'static str {
    match value {
        ClassificationCategoryRecord::ProductionCode => "production_code",
        ClassificationCategoryRecord::Test => "test",
        ClassificationCategoryRecord::Documentation => "documentation",
        ClassificationCategoryRecord::CiCd => "ci_cd",
        ClassificationCategoryRecord::Infrastructure => "infrastructure",
        ClassificationCategoryRecord::SchemaMigration => "schema_migration",
        ClassificationCategoryRecord::Dependency => "dependency",
        ClassificationCategoryRecord::SecurityPolicy => "security_policy",
        ClassificationCategoryRecord::Configuration => "configuration",
        ClassificationCategoryRecord::Generated => "generated",
        ClassificationCategoryRecord::Vendored => "vendored",
        ClassificationCategoryRecord::BuildOutput => "build_output",
        ClassificationCategoryRecord::Coverage => "coverage",
        ClassificationCategoryRecord::Unknown => "unknown",
    }
}

pub(crate) const fn classification_tag_code(value: ClassificationTagRecord) -> &'static str {
    match value {
        ClassificationTagRecord::DependencyManifest => "dependency_manifest",
        ClassificationTagRecord::Lockfile => "lockfile",
        ClassificationTagRecord::LinguistGenerated => "linguist_generated",
        ClassificationTagRecord::LinguistVendored => "linguist_vendored",
        ClassificationTagRecord::GeneratedSuppressed => "generated_suppressed",
        ClassificationTagRecord::VendoredSuppressed => "vendored_suppressed",
        ClassificationTagRecord::AttributesUnavailable => "attributes_unavailable",
        ClassificationTagRecord::Minified => "minified",
    }
}

pub(crate) const fn classification_evidence_kind_code(
    value: ClassificationEvidenceKindRecord,
) -> &'static str {
    match value {
        ClassificationEvidenceKindRecord::PolicyRule => "policy_rule",
        ClassificationEvidenceKindRecord::LinguistAttribute => "linguist_attribute",
        ClassificationEvidenceKindRecord::AttributeFactsUnavailable => {
            "attribute_facts_unavailable"
        }
    }
}

pub(crate) const fn evidence_status_code(value: EvidenceStatus) -> &'static str {
    match value {
        EvidenceStatus::Complete => "complete",
        EvidenceStatus::Partial => "partial",
        EvidenceStatus::Unavailable => "unavailable",
        EvidenceStatus::Unsupported => "unsupported",
        EvidenceStatus::Insufficient => "insufficient",
        EvidenceStatus::Pending => "pending",
    }
}
