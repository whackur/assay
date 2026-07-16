use crate::{
    audit::AuditEvent, entitlements::LocalRole, values::ClaimName, verification::VerifiedIdentity,
};

/// One trusted upstream claim that a deployment maps to Assay Administrator.
///
/// The trusted value is deployment configuration, not an imported provider role
/// enum. Assay compares claim values as opaque strings and never queries the
/// upstream user database.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TrustedAdminClaim {
    claim_name: ClaimName,
    value: String,
}

impl TrustedAdminClaim {
    /// Declares a trusted `(claim_name, value)` administrator mapping.
    pub fn new(claim_name: ClaimName, value: &str) -> Self {
        Self {
            claim_name,
            value: value.to_owned(),
        }
    }

    /// Returns the claim name inspected for the trusted value.
    pub const fn claim_name(&self) -> &ClaimName {
        &self.claim_name
    }
}

/// How a local role was assigned, recorded for authorization and audit.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RoleSource {
    DefaultMember,
    MappedAdministrator { matched_claim: ClaimName },
}

/// The result of evaluating identity against the administrator mapping policy.
#[derive(Clone, Debug)]
pub struct RoleAssignment {
    role: LocalRole,
    source: RoleSource,
    audit: Option<AuditEvent>,
}

impl RoleAssignment {
    /// Returns the assigned local role.
    pub const fn role(&self) -> LocalRole {
        self.role
    }

    /// Returns the provenance of the assignment.
    pub const fn source(&self) -> &RoleSource {
        &self.source
    }

    /// Returns the audit event emitted for a privileged mapping.
    pub const fn audit(&self) -> Option<&AuditEvent> {
        self.audit.as_ref()
    }
}

/// Explicit deployment policy mapping trusted upstream claims to Administrator.
///
/// An empty policy grants no administrator; an external role alone never elevates.
#[derive(Clone, Debug)]
pub struct AdministratorMappingPolicy {
    version: String,
    trusted_claims: Vec<TrustedAdminClaim>,
}

impl AdministratorMappingPolicy {
    /// Builds a versioned policy from explicit trusted claims.
    pub fn new(version: &str, trusted_claims: Vec<TrustedAdminClaim>) -> Self {
        Self {
            version: version.to_owned(),
            trusted_claims,
        }
    }

    /// Builds a policy that never maps any claim to Administrator.
    pub fn none(version: &str) -> Self {
        Self::new(version, Vec::new())
    }

    /// Returns the policy version stamped onto audit records.
    pub fn version(&self) -> &str {
        &self.version
    }

    /// Assigns a local role, emitting an audit event for a privileged mapping.
    pub fn assign(&self, identity: &VerifiedIdentity) -> RoleAssignment {
        for trusted in &self.trusted_claims {
            let present = identity
                .claims()
                .claim_values(trusted.claim_name())
                .iter()
                .any(|value| value == &trusted.value);
            if present {
                let matched = trusted.claim_name().clone();
                let audit = AuditEvent::administrator_mapping(
                    identity.account_key().clone(),
                    matched.clone(),
                    &self.version,
                );
                return RoleAssignment {
                    role: LocalRole::Administrator,
                    source: RoleSource::MappedAdministrator {
                        matched_claim: matched,
                    },
                    audit: Some(audit),
                };
            }
        }
        RoleAssignment {
            role: LocalRole::Member,
            source: RoleSource::DefaultMember,
            audit: None,
        }
    }
}
