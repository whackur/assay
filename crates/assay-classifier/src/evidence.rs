//! Non-sensitive provenance retained by a classification result.

use crate::identifiers::RuleId;

/// Kind of provenance retained by a classification result.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClassificationEvidenceKind {
    /// A named versioned policy rule matched input facts.
    PolicyRule,
    /// A resolved `.gitattributes` Linguist value was applied.
    LinguistAttribute,
    /// Attribute resolution was explicitly unavailable.
    AttributeFactsUnavailable,
}

/// Non-sensitive provenance for a classification decision.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClassificationEvidence {
    kind: ClassificationEvidenceKind,
    rule_id: RuleId,
    attribute_name: Option<&'static str>,
    attribute_value: Option<bool>,
}

impl ClassificationEvidence {
    /// Creates non-sensitive evidence for an external versioned policy rule.
    pub fn policy_rule(rule_id: RuleId) -> Self {
        Self {
            kind: ClassificationEvidenceKind::PolicyRule,
            rule_id,
            attribute_name: None,
            attribute_value: None,
        }
    }

    pub(crate) fn attribute(rule_id: RuleId, name: &'static str, value: bool) -> Self {
        Self {
            kind: ClassificationEvidenceKind::LinguistAttribute,
            rule_id,
            attribute_name: Some(name),
            attribute_value: Some(value),
        }
    }

    pub(crate) fn unavailable() -> Self {
        Self {
            kind: ClassificationEvidenceKind::AttributeFactsUnavailable,
            rule_id: RuleId::built_in("classifier.v1.attributes.unavailable"),
            attribute_name: None,
            attribute_value: None,
        }
    }

    /// Returns the provenance kind.
    pub const fn kind(&self) -> ClassificationEvidenceKind {
        self.kind
    }

    /// Returns the rule that supplied this evidence.
    pub const fn rule_id(&self) -> &RuleId {
        &self.rule_id
    }

    /// Returns a Linguist attribute name for attribute evidence.
    pub const fn attribute_name(&self) -> Option<&'static str> {
        self.attribute_name
    }

    /// Returns a Linguist attribute value for attribute evidence.
    pub const fn attribute_value(&self) -> Option<bool> {
        self.attribute_value
    }

    /// Returns true when this evidence preserves unavailable attribute facts.
    pub const fn is_unavailable(&self) -> bool {
        matches!(
            self.kind,
            ClassificationEvidenceKind::AttributeFactsUnavailable
        )
    }
}
