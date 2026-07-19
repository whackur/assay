//! Policy adapter boundary and the version-preserving classification entry point.

use std::collections::BTreeSet;

use crate::{
    attributes::{AttributeAvailability, LinguistAttributeFacts},
    categories::ClassificationTag,
    decision::{ClassificationDecision, FileClassification},
    evidence::ClassificationEvidence,
    identifiers::RuleId,
    input::FileClassificationInput,
};

/// Adapter boundary for built-in or externally configured versioned policy.
///
/// Future repository, organization, or deployment policy adapters implement
/// this trait. The built-in policy intentionally contains no project-specific
/// names or organization-specific exceptions.
pub trait ClassificationPolicy {
    /// Returns this policy's validated, explicit version identity.
    fn policy_version(&self) -> crate::identifiers::PolicyVersion;

    /// Evaluates a validated file input without I/O.
    fn evaluate(&self, input: &FileClassificationInput) -> ClassificationDecision;
}

/// Evaluates a policy and attaches its validated version to the result.
///
/// This is the enforced entry point for external policies: implementations
/// return only a [`ClassificationDecision`], while this function preserves the
/// policy identity, canonical Linguist facts, and input availability in
/// [`FileClassification`]. Canonical evidence and tags are deduplicated in a
/// stable order.
pub fn classify_with_policy(
    policy: &(impl ClassificationPolicy + ?Sized),
    input: &FileClassificationInput,
) -> FileClassification {
    let decision = attach_input_provenance(policy.evaluate(input), input.attributes());
    FileClassification::from_decision(
        policy.policy_version(),
        decision,
        input.attributes().availability(),
    )
}

pub(crate) fn attach_input_provenance(
    mut decision: ClassificationDecision,
    attributes: LinguistAttributeFacts,
) -> ClassificationDecision {
    let mut tags = decision.tags.into_iter().collect::<BTreeSet<_>>();
    let mut evidence = decision.evidence;
    match attributes.availability() {
        AttributeAvailability::Available => {
            if let Some(value) = attributes.generated() {
                push_unique_evidence(
                    &mut evidence,
                    ClassificationEvidence::attribute(
                        RuleId::built_in("classifier.v1.attribute.generated"),
                        "linguist-generated",
                        value,
                    ),
                );
                if value {
                    tags.insert(ClassificationTag::LinguistGenerated);
                }
            }
            if let Some(value) = attributes.vendored() {
                push_unique_evidence(
                    &mut evidence,
                    ClassificationEvidence::attribute(
                        RuleId::built_in("classifier.v1.attribute.vendored"),
                        "linguist-vendored",
                        value,
                    ),
                );
                if value {
                    tags.insert(ClassificationTag::LinguistVendored);
                }
            }
        }
        AttributeAvailability::Unavailable => {
            tags.insert(ClassificationTag::AttributesUnavailable);
            push_unique_evidence(&mut evidence, ClassificationEvidence::unavailable());
        }
    }
    if !evidence
        .iter()
        .any(|item| item.rule_id() == &decision.rule_id)
    {
        evidence.push(ClassificationEvidence::policy_rule(
            decision.rule_id.clone(),
        ));
    }
    decision.tags = tags.into_iter().collect();
    decision.evidence = evidence;
    decision
}

fn push_unique_evidence(evidence: &mut Vec<ClassificationEvidence>, item: ClassificationEvidence) {
    if !evidence.contains(&item) {
        evidence.push(item);
    }
}
