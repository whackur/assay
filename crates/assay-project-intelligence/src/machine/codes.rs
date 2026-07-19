use assay_domain::{AnalysisStatus, EvidenceStatus};
use assay_git::{EntryMode, ObjectKind};

use crate::ClassificationAvailabilityReason;
use crate::ClassificationCategoryRecord;
use crate::ClassificationEvidenceKindRecord;
use crate::ClassificationTagRecord;
use crate::PortablePathEncoding;
use crate::RawEvidenceIssue;
use crate::RawEvidenceKind;

pub(crate) fn source_kind(kind: RawEvidenceKind) -> &'static str {
    match kind {
        RawEvidenceKind::RepositorySnapshot => "repository",
        RawEvidenceKind::TrackedFile => "repository_content",
        RawEvidenceKind::HistoryScope | RawEvidenceKind::ParentDelta => "repository_history",
    }
}

pub(crate) fn evidence_status(status: EvidenceStatus) -> &'static str {
    match status {
        EvidenceStatus::Complete => "complete",
        EvidenceStatus::Partial => "partial",
        EvidenceStatus::Unavailable => "unavailable",
        EvidenceStatus::Unsupported => "unsupported",
        EvidenceStatus::Insufficient => "insufficient",
        EvidenceStatus::Pending => "pending",
    }
}

pub(crate) fn analysis_status(status: AnalysisStatus) -> &'static str {
    match status {
        AnalysisStatus::Complete => "complete",
        AnalysisStatus::Partial => "partial",
        AnalysisStatus::Unavailable => "unavailable",
        AnalysisStatus::Unsupported => "unsupported",
        AnalysisStatus::Insufficient => "insufficient",
        AnalysisStatus::Pending => "pending",
    }
}

pub(crate) fn path_encoding(value: PortablePathEncoding) -> &'static str {
    match value {
        PortablePathEncoding::Utf8 => "utf8",
        PortablePathEncoding::GitPathHex => "git_path_hex",
    }
}

pub(crate) fn entry_mode(value: EntryMode) -> &'static str {
    match value {
        EntryMode::Regular => "regular",
        EntryMode::Executable => "executable",
        EntryMode::SymbolicLink => "symbolic_link",
        EntryMode::Gitlink => "gitlink",
    }
}

pub(crate) fn object_kind(value: ObjectKind) -> &'static str {
    match value {
        ObjectKind::Blob => "blob",
        ObjectKind::Commit => "commit",
    }
}

pub(crate) fn language(
    encoding: PortablePathEncoding,
    path: &str,
) -> (Option<&'static str>, &'static str) {
    if encoding != PortablePathEncoding::Utf8 {
        return (None, "unsupported");
    }
    match path
        .rsplit('.')
        .next()
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("js" | "mjs" | "cjs") => (Some("JavaScript"), "complete"),
        Some("ts") => (Some("TypeScript"), "complete"),
        Some("tsx") => (Some("TSX"), "complete"),
        Some("py") => (Some("Python"), "complete"),
        _ => (None, "unsupported"),
    }
}

pub(crate) fn raw_issue(value: RawEvidenceIssue) -> &'static str {
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

pub(crate) fn classification_reason(value: ClassificationAvailabilityReason) -> &'static str {
    match value {
        ClassificationAvailabilityReason::AttributesUnavailable => "attributes_unavailable",
        ClassificationAvailabilityReason::MissingClassification => "missing_classification",
        ClassificationAvailabilityReason::NonPortablePath => "non_portable_path",
    }
}

pub(crate) fn category(value: ClassificationCategoryRecord) -> &'static str {
    match value {
        ClassificationCategoryRecord::ProductionCode => "production_code",
        ClassificationCategoryRecord::Test => "test",
        ClassificationCategoryRecord::Documentation => "documentation",
        ClassificationCategoryRecord::CiCd => "ci_cd",
        ClassificationCategoryRecord::Infrastructure => "infrastructure",
        ClassificationCategoryRecord::SchemaMigration => "schema_migration",
        ClassificationCategoryRecord::Dependency => "dependency",
        ClassificationCategoryRecord::SecurityPolicy => "security",
        ClassificationCategoryRecord::Configuration => "configuration",
        ClassificationCategoryRecord::Generated => "generated",
        ClassificationCategoryRecord::Vendored => "vendored",
        ClassificationCategoryRecord::BuildOutput => "build_output",
        ClassificationCategoryRecord::Coverage => "coverage",
        ClassificationCategoryRecord::Unknown => "unknown",
    }
}

pub(crate) fn tag(value: ClassificationTagRecord) -> Option<&'static str> {
    match value {
        ClassificationTagRecord::DependencyManifest => Some("dependency"),
        ClassificationTagRecord::Lockfile => Some("lockfile"),
        ClassificationTagRecord::LinguistGenerated => Some("generated"),
        ClassificationTagRecord::LinguistVendored => Some("vendored"),
        ClassificationTagRecord::Minified => Some("minified"),
        ClassificationTagRecord::GeneratedSuppressed
        | ClassificationTagRecord::VendoredSuppressed
        | ClassificationTagRecord::AttributesUnavailable => None,
    }
}

pub(crate) fn classification_evidence_kind(
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
