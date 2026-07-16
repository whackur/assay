mod common;

use std::str::FromStr;

use assay_identity::{
    AdministratorMappingPolicy, AuditAction, ClaimName, Entitlement, EntitlementPolicy, LocalRole,
    RoleSource, SigningAlgorithm, TokenValidator, UpstreamIdToken, VerifiedClaimsBuilder,
    VerifiedIdentity,
};
use common::{AUDIENCE, FakeVerifier, FixedClock, ISSUER, config};

const NOW: i64 = 1_000_000;
const NONCE: &str = "n";

fn identity_with_role_claim(claim: &str, value: &str) -> VerifiedIdentity {
    let claims = VerifiedClaimsBuilder::new(ISSUER, "subject-123", NOW + 300, NOW)
        .audience(AUDIENCE)
        .nonce(NONCE)
        .string_claim(claim, value)
        .build();
    let config = config();
    let validator = TokenValidator::new(&config);
    let verifier = FakeVerifier::proving(SigningAlgorithm::Rs256, claims);
    let token = UpstreamIdToken::new("h.p.s".to_owned());
    validator
        .validate(&verifier, &token, NONCE, &FixedClock(NOW))
        .unwrap()
}

fn admin_policy() -> AdministratorMappingPolicy {
    AdministratorMappingPolicy::new(
        "hakhub-admin-map-1",
        vec![assay_identity::TrustedAdminClaim::new(
            ClaimName::from_str("hakhub_role").unwrap(),
            "admin",
        )],
    )
}

#[test]
fn an_external_admin_role_alone_does_not_grant_assay_administrator() {
    let identity = identity_with_role_claim("hakhub_role", "admin");
    let assignment = AdministratorMappingPolicy::none("empty-1").assign(&identity);
    assert_eq!(assignment.role(), LocalRole::Member);
    assert!(assignment.audit().is_none());

    let entitlements = EntitlementPolicy::default();
    assert!(!entitlements.authorizes(assignment.role(), Entitlement::AnalysisAdminRerun));
}

#[test]
fn an_explicit_policy_maps_the_trusted_claim_and_audits_it() {
    let identity = identity_with_role_claim("hakhub_role", "admin");
    let assignment = admin_policy().assign(&identity);

    assert_eq!(assignment.role(), LocalRole::Administrator);
    match assignment.source() {
        RoleSource::MappedAdministrator { matched_claim } => {
            assert_eq!(matched_claim.as_str(), "hakhub_role");
        }
        other => panic!("expected a mapped administrator, got {other:?}"),
    }

    let audit = assignment.audit().expect("privileged mapping is auditable");
    assert_eq!(audit.action(), AuditAction::AdministratorMappingApplied);
    assert_eq!(audit.matched_claim().as_str(), "hakhub_role");
    assert_eq!(audit.policy_version(), "hakhub-admin-map-1");
    assert_eq!(audit.account_key().subject().as_str(), "subject-123");

    let entitlements = EntitlementPolicy::default();
    assert!(entitlements.authorizes(assignment.role(), Entitlement::AnalysisAdminRerun));
}

#[test]
fn a_non_matching_claim_stays_a_member() {
    let identity = identity_with_role_claim("hakhub_role", "member");
    let assignment = admin_policy().assign(&identity);
    assert_eq!(assignment.role(), LocalRole::Member);
    assert!(assignment.audit().is_none());
}

#[test]
fn the_audit_event_serializes_without_secret_values() {
    let identity = identity_with_role_claim("hakhub_role", "admin");
    let assignment = admin_policy().assign(&identity);
    let audit = assignment.audit().unwrap();
    let json = serde_json::to_string(audit).unwrap();
    assert!(json.contains("administrator_mapping_applied"));
    assert!(json.contains("hakhub_role"));
    assert!(json.contains("subject-123"));
    assert!(!json.to_lowercase().contains("token"));
    assert!(!json.contains("h.p.s"));
}
