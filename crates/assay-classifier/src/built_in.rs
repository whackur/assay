//! Built-in Assay file classification policy.

use std::collections::BTreeSet;

use crate::{
    attributes::AttributeAvailability,
    categories::{ClassificationCategory, ClassificationTag},
    confidence::Confidence,
    decision::ClassificationDecision,
    identifiers::PolicyVersion,
    input::FileClassificationInput,
    policy::ClassificationPolicy,
    rules::classify_path_v1,
};

/// Built-in Assay file classification policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BuiltInPolicy {
    /// Initial versioned path and Linguist-attribute policy.
    V1,
}

impl BuiltInPolicy {
    /// Returns the stable version for this policy.
    pub fn version(self) -> PolicyVersion {
        match self {
            Self::V1 => PolicyVersion::built_in(),
        }
    }

    /// Classifies one file through the same version-preserving entry point
    /// used by external policies.
    pub fn classify(self, input: &FileClassificationInput) -> crate::FileClassification {
        crate::classify_with_policy(&self, input)
    }
}

impl ClassificationPolicy for BuiltInPolicy {
    fn policy_version(&self) -> PolicyVersion {
        self.version()
    }

    fn evaluate(&self, input: &FileClassificationInput) -> ClassificationDecision {
        match self {
            Self::V1 => evaluate_v1(input),
        }
    }
}

fn evaluate_v1(input: &FileClassificationInput) -> ClassificationDecision {
    let attributes = input.attributes();
    let mut tags = BTreeSet::new();

    let generated_rules_enabled = attributes.generated() != Some(false);
    let vendored_rules_enabled = attributes.vendored() != Some(false);
    let path_decision = classify_path_v1(
        input.path(),
        generated_rules_enabled,
        vendored_rules_enabled,
    );

    // When both attributes are true, generated is the deterministic primary
    // category and vendored remains visible as a secondary tag and evidence.
    let decision = if attributes.generated() == Some(true) {
        ClassificationDecision::built_in(
            ClassificationCategory::Generated,
            "classifier.v1.attribute.generated",
            Confidence::CERTAIN,
        )
    } else if attributes.vendored() == Some(true) {
        ClassificationDecision::built_in(
            ClassificationCategory::Vendored,
            "classifier.v1.attribute.vendored",
            Confidence::CERTAIN,
        )
    } else {
        path_decision.clone()
    };

    if attributes.generated() == Some(false)
        && classify_path_v1(input.path(), true, vendored_rules_enabled).category
            == ClassificationCategory::Generated
        && path_decision.category != ClassificationCategory::Generated
    {
        tags.insert(ClassificationTag::GeneratedSuppressed);
    }
    if attributes.vendored() == Some(false)
        && classify_path_v1(input.path(), generated_rules_enabled, true).category
            == ClassificationCategory::Vendored
        && path_decision.category != ClassificationCategory::Vendored
    {
        tags.insert(ClassificationTag::VendoredSuppressed);
    }

    tags.extend(decision.tags);
    let confidence = if matches!(
        attributes.availability(),
        AttributeAvailability::Unavailable
    ) {
        decision.confidence.min(Confidence::MEDIUM)
    } else {
        decision.confidence
    };

    ClassificationDecision {
        category: decision.category,
        tags: tags.into_iter().collect(),
        rule_id: decision.rule_id,
        confidence,
        evidence: Vec::new(),
    }
}
