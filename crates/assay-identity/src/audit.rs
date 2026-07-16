use serde::{Deserialize, Serialize};

use crate::values::{AccountKey, ClaimName};

/// A security- and privacy-relevant action recorded without any secret value.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditAction {
    AdministratorMappingApplied,
}

/// An audit record for a privileged mapping. It carries no token or secret material.
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

    /// Returns the recorded action.
    pub const fn action(&self) -> AuditAction {
        self.action
    }

    /// Returns the account the privileged mapping applied to.
    pub const fn account_key(&self) -> &AccountKey {
        &self.account_key
    }

    /// Returns the name of the claim that matched the trusted mapping.
    pub const fn matched_claim(&self) -> &ClaimName {
        &self.matched_claim
    }

    /// Returns the deployment policy version that authorized the mapping.
    pub fn policy_version(&self) -> &str {
        &self.policy_version
    }
}
