use assay_classifier::{
    ClassificationCategory, ClassificationEvidenceKind, ClassificationTag, FileClassification,
};
use assay_domain::EvidenceStatus;
use assay_git::{HistoryIssue, ObjectIssue, ParentDeltaIssue};

use crate::evidence::classification_record::{ClassificationEvidenceFact, ClassificationPayload};
use crate::evidence::types::{
    ClassificationCategoryRecord, ClassificationEvidenceKindRecord, ClassificationTagRecord,
    RawEvidenceIssue,
};

pub(crate) fn map_classification(classification: &FileClassification) -> ClassificationPayload {
    let mut tags = classification
        .tags()
        .iter()
        .copied()
        .map(map_classification_tag)
        .collect::<Vec<_>>();
    tags.sort_unstable();
    tags.dedup();
    let mut evidence = classification
        .evidence()
        .iter()
        .map(|item| ClassificationEvidenceFact {
            kind: match item.kind() {
                ClassificationEvidenceKind::PolicyRule => {
                    ClassificationEvidenceKindRecord::PolicyRule
                }
                ClassificationEvidenceKind::LinguistAttribute => {
                    ClassificationEvidenceKindRecord::LinguistAttribute
                }
                ClassificationEvidenceKind::AttributeFactsUnavailable => {
                    ClassificationEvidenceKindRecord::AttributeFactsUnavailable
                }
            },
            rule_id: item.rule_id().as_str().to_owned(),
            attribute_name: item.attribute_name(),
            attribute_value: item.attribute_value(),
        })
        .collect::<Vec<_>>();
    evidence.sort();
    evidence.dedup();
    ClassificationPayload {
        category: map_classification_category(classification.category()),
        tags,
        rule_id: classification.rule_id().as_str().to_owned(),
        confidence_basis_points: classification.confidence().basis_points(),
        evidence,
    }
}

fn map_classification_category(value: ClassificationCategory) -> ClassificationCategoryRecord {
    match value {
        ClassificationCategory::ProductionCode => ClassificationCategoryRecord::ProductionCode,
        ClassificationCategory::Test => ClassificationCategoryRecord::Test,
        ClassificationCategory::Documentation => ClassificationCategoryRecord::Documentation,
        ClassificationCategory::CiCd => ClassificationCategoryRecord::CiCd,
        ClassificationCategory::Infrastructure => ClassificationCategoryRecord::Infrastructure,
        ClassificationCategory::SchemaMigration => ClassificationCategoryRecord::SchemaMigration,
        ClassificationCategory::Dependency => ClassificationCategoryRecord::Dependency,
        ClassificationCategory::SecurityPolicy => ClassificationCategoryRecord::SecurityPolicy,
        ClassificationCategory::Configuration => ClassificationCategoryRecord::Configuration,
        ClassificationCategory::Generated => ClassificationCategoryRecord::Generated,
        ClassificationCategory::Vendored => ClassificationCategoryRecord::Vendored,
        ClassificationCategory::BuildOutput => ClassificationCategoryRecord::BuildOutput,
        ClassificationCategory::Coverage => ClassificationCategoryRecord::Coverage,
        ClassificationCategory::Unknown => ClassificationCategoryRecord::Unknown,
    }
}

fn map_classification_tag(value: ClassificationTag) -> ClassificationTagRecord {
    match value {
        ClassificationTag::DependencyManifest => ClassificationTagRecord::DependencyManifest,
        ClassificationTag::Lockfile => ClassificationTagRecord::Lockfile,
        ClassificationTag::LinguistGenerated => ClassificationTagRecord::LinguistGenerated,
        ClassificationTag::LinguistVendored => ClassificationTagRecord::LinguistVendored,
        ClassificationTag::GeneratedSuppressed => ClassificationTagRecord::GeneratedSuppressed,
        ClassificationTag::VendoredSuppressed => ClassificationTagRecord::VendoredSuppressed,
        ClassificationTag::AttributesUnavailable => ClassificationTagRecord::AttributesUnavailable,
        ClassificationTag::Minified => ClassificationTagRecord::Minified,
    }
}

pub(crate) fn map_object_issue(value: ObjectIssue) -> RawEvidenceIssue {
    match value {
        ObjectIssue::GitlinkContent => RawEvidenceIssue::GitlinkContent,
        ObjectIssue::SizeLimit => RawEvidenceIssue::SizeLimit,
        ObjectIssue::MissingOrUnreadable => RawEvidenceIssue::MissingOrUnreadable,
        ObjectIssue::Timeout => RawEvidenceIssue::Timeout,
        ObjectIssue::OutputLimit => RawEvidenceIssue::OutputLimit,
        ObjectIssue::MalformedMetadata => RawEvidenceIssue::MalformedMetadata,
    }
}

pub(crate) fn map_history_issue(value: HistoryIssue) -> RawEvidenceIssue {
    match value {
        HistoryIssue::DepthLimit => RawEvidenceIssue::HistoryDepthLimit,
        HistoryIssue::ShallowRepository => RawEvidenceIssue::ShallowRepository,
        HistoryIssue::ProcessFailure => RawEvidenceIssue::ProcessFailure,
        HistoryIssue::MalformedOutput => RawEvidenceIssue::MalformedOutput,
    }
}

pub(crate) fn map_parent_delta_issue(value: ParentDeltaIssue) -> RawEvidenceIssue {
    match value {
        ParentDeltaIssue::RenameCandidateLimit => RawEvidenceIssue::RenameCandidateLimit,
        ParentDeltaIssue::ShallowRepository => RawEvidenceIssue::ShallowRepository,
        ParentDeltaIssue::ProcessFailure => RawEvidenceIssue::ProcessFailure,
        ParentDeltaIssue::MalformedOutput => RawEvidenceIssue::MalformedOutput,
    }
}

pub(crate) fn parent_delta_values(
    status: EvidenceStatus,
    issue: Option<ParentDeltaIssue>,
    changed_entries: usize,
    renames: usize,
) -> (Option<usize>, Option<usize>) {
    match (status, issue) {
        (EvidenceStatus::Complete, None) => (Some(changed_entries), Some(renames)),
        (EvidenceStatus::Partial, Some(ParentDeltaIssue::RenameCandidateLimit)) => {
            (Some(changed_entries), None)
        }
        _ => (None, None),
    }
}
