use serde::{Deserialize, Serialize};

use crate::values::{AccountKey, ClaimName};

/// Security- and privacy-relevant action recorded without any secret value.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditAction {
    AdministratorMappingApplied,
}

/// Audit record for a privileged mapping. Carries no token or secret material.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AuditEvent {
    action: AuditAction,
    account_key: AccountKey,
    matched_claim: ClaimName,
    policy_version: String,
}

impl AuditEvent {
    /// Records an administrator mapping keyed by account, matched claim, and policy version.
    pub fn administrator_mapping(
        account_key: AccountKey,
        matched_claim: ClaimName,
        policy_version: &str,
    ) -> Self {
        Self {
            action: AuditAction::AdministratorMappingApplied,
            account_key,
            matched_claim,
            policy_version: policy_version.to_owned(),
        }
    }

    pub const fn action(&self) -> AuditAction {
        self.action
    }

    /// Account the privileged mapping applied to.
    pub const fn account_key(&self) -> &AccountKey {
        &self.account_key
    }

    /// Name of the claim that matched the trusted mapping.
    pub const fn matched_claim(&self) -> &ClaimName {
        &self.matched_claim
    }

    /// Deployment policy version that authorized the mapping.
    pub fn policy_version(&self) -> &str {
        &self.policy_version
    }
}
