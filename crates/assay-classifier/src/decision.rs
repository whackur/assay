//! Classification decision and complete result types.
//!
//! Split from `lib.rs` so the output contract stays separate from the policy
//! evaluation that produces it. `ClassificationDecision` is the explainable
//! intermediate value returned by policy adapters; `FileClassification` is the
//! versioned, provenance-attached final result.

use std::{collections::BTreeSet, fmt};

use crate::{
    attributes::AttributeAvailability, categories::ClassificationCategory, confidence::Confidence,
    evidence::ClassificationEvidence, identifiers::RuleId,
};

/// Complete, explainable classification of one file.
///
/// This output measures policy evidence only. It cannot establish source
/// correctness, value, intent, semantic impact, or contributor performance.
#[derive(Clone, Eq, PartialEq)]
pub struct FileClassification {
    policy_version: crate::identifiers::PolicyVersion,
    category: ClassificationCategory,
    tags: Vec<crate::categories::ClassificationTag>,
    rule_id: RuleId,
    confidence: Confidence,
    evidence: Vec<ClassificationEvidence>,
    attribute_availability: AttributeAvailability,
}

/// Explainable decision returned by a classification policy.
///
/// The decision does not carry policy identity itself. Callers use
/// [`crate::classify_with_policy`] so the policy's validated version is attached to
/// the final result and cannot be omitted by a policy implementation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClassificationDecision {
    pub(crate) category: ClassificationCategory,
    pub(crate) tags: Vec<crate::categories::ClassificationTag>,
    pub(crate) rule_id: RuleId,
    pub(crate) confidence: Confidence,
    pub(crate) evidence: Vec<ClassificationEvidence>,
}

impl ClassificationDecision {
    /// Creates an explainable decision for a versioned policy adapter.
    ///
    /// Tags are sorted and deduplicated. The primary policy rule is retained
    /// as evidence automatically if the adapter does not supply it.
    pub fn new(
        category: ClassificationCategory,
        tags: impl IntoIterator<Item = crate::categories::ClassificationTag>,
        rule_id: RuleId,
        confidence: Confidence,
        evidence: impl IntoIterator<Item = ClassificationEvidence>,
    ) -> Self {
        let tags = tags
            .into_iter()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        let mut evidence = evidence.into_iter().collect::<Vec<_>>();
        if !evidence.iter().any(|item| item.rule_id() == &rule_id) {
            evidence.push(ClassificationEvidence::policy_rule(rule_id.clone()));
        }
        Self {
            category,
            tags,
            rule_id,
            confidence,
            evidence,
        }
    }

    pub(crate) fn built_in(
        category: ClassificationCategory,
        rule_id: &'static str,
        confidence: Confidence,
    ) -> Self {
        Self {
            category,
            tags: Vec::new(),
            rule_id: RuleId::built_in(rule_id),
            confidence,
            evidence: Vec::new(),
        }
    }

    pub(crate) fn tagged(mut self, tag: crate::categories::ClassificationTag) -> Self {
        self.tags.push(tag);
        self
    }
}

impl FileClassification {
    pub(crate) fn from_decision(
        policy_version: crate::identifiers::PolicyVersion,
        decision: ClassificationDecision,
        attribute_availability: AttributeAvailability,
    ) -> Self {
        Self {
            policy_version,
            category: decision.category,
            tags: decision.tags,
            rule_id: decision.rule_id,
            confidence: decision.confidence,
            evidence: decision.evidence,
            attribute_availability,
        }
    }

    /// Returns the validated identity of the complete policy that produced
    /// this result.
    pub const fn policy_version(&self) -> &crate::identifiers::PolicyVersion {
        &self.policy_version
    }

    /// Returns the single primary category.
    pub const fn category(&self) -> ClassificationCategory {
        self.category
    }

    /// Returns stable, sorted secondary tags.
    pub fn tags(&self) -> &[crate::categories::ClassificationTag] {
        &self.tags
    }

    /// Returns the primary versioned rule identifier.
    pub const fn rule_id(&self) -> &RuleId {
        &self.rule_id
    }

    /// Returns policy confidence, not a quality score.
    pub const fn confidence(&self) -> Confidence {
        self.confidence
    }

    /// Returns non-sensitive rule and attribute provenance.
    pub fn evidence(&self) -> &[ClassificationEvidence] {
        &self.evidence
    }

    /// Returns whether resolved Git attribute facts were available.
    pub const fn attribute_availability(&self) -> AttributeAvailability {
        self.attribute_availability
    }
}

impl fmt::Debug for FileClassification {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("FileClassification")
            .field("policy_version", &self.policy_version)
            .field("category", &self.category)
            .field("tags", &self.tags)
            .field("rule_id", &self.rule_id)
            .field("confidence", &self.confidence)
            .field("evidence", &self.evidence)
            .field("attribute_availability", &self.attribute_availability)
            .finish()
    }
}
