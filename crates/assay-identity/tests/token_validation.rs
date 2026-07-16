mod common;

use std::str::FromStr;

use assay_identity::{
    ClaimName, SignatureError, SigningAlgorithm, TokenValidator, UpstreamIdToken, ValidationError,
    VerifiedClaims, VerifiedClaimsBuilder,
};
use common::{AUDIENCE, CLIENT_ID, FakeVerifier, FixedClock, ISSUER, config};

const NOW: i64 = 1_000_000;
const NONCE: &str = "transaction-nonce";

fn base_claims() -> VerifiedClaimsBuilder {
    VerifiedClaimsBuilder::new(ISSUER, "subject-123", NOW + 300, NOW - 5)
        .audience(AUDIENCE)
        .nonce(NONCE)
}

fn token() -> UpstreamIdToken {
    UpstreamIdToken::new("header.payload.signature".to_owned())
}

fn validate(claims: VerifiedClaims) -> Result<assay_identity::VerifiedIdentity, ValidationError> {
    let config = config();
    let validator = TokenValidator::new(&config);
    let verifier = FakeVerifier::proving(SigningAlgorithm::Rs256, claims);
    validator.validate(&verifier, &token(), NONCE, &FixedClock(NOW))
}

#[test]
fn valid_token_yields_issuer_subject_account_key() {
    let identity = validate(base_claims().build()).unwrap();
    assert_eq!(identity.account_key().issuer().as_str(), ISSUER);
    assert_eq!(identity.account_key().subject().as_str(), "subject-123");
    assert_eq!(identity.authentication_time(), NOW - 5);
}

#[test]
fn email_change_keeps_the_same_account_and_never_merges_unrelated_ones() {
    let with_first = validate(
        base_claims()
            .string_claim("email", "old@example.com")
            .build(),
    )
    .unwrap()
    .account_key()
    .clone();
    let with_second = validate(
        base_claims()
            .string_claim("email", "new@example.com")
            .build(),
    )
    .unwrap()
    .account_key()
    .clone();
    assert_eq!(
        with_first, with_second,
        "email is not part of the account key"
    );

    let other_subject = VerifiedClaimsBuilder::new(ISSUER, "subject-999", NOW + 300, NOW - 5)
        .audience(AUDIENCE)
        .nonce(NONCE)
        .string_claim("email", "old@example.com")
        .build();
    let different = validate(other_subject).unwrap().account_key().clone();
    assert_ne!(
        with_first, different,
        "a shared email must not merge distinct subjects"
    );
}

#[test]
fn issuer_mismatch_fails_closed() {
    let claims = VerifiedClaimsBuilder::new("https://evil.example", "subject-123", NOW + 300, NOW)
        .audience(AUDIENCE)
        .nonce(NONCE)
        .build();
    assert_eq!(validate(claims), Err(ValidationError::IssuerMismatch));
}

#[test]
fn token_for_another_audience_only_is_rejected() {
    let claims = VerifiedClaimsBuilder::new(ISSUER, "subject-123", NOW + 300, NOW)
        .audience("hakhub-web")
        .nonce(NONCE)
        .build();
    assert_eq!(validate(claims), Err(ValidationError::AudienceMismatch));
}

#[test]
fn multiple_audiences_require_matching_authorized_party() {
    let missing_azp = base_claims().audience("another-api").build();
    assert_eq!(
        validate(missing_azp),
        Err(ValidationError::AuthorizedPartyMismatch)
    );

    let wrong_azp = base_claims()
        .audience("another-api")
        .authorized_party("someone-else")
        .build();
    assert_eq!(
        validate(wrong_azp),
        Err(ValidationError::AuthorizedPartyMismatch)
    );

    let correct = base_claims()
        .audience("another-api")
        .authorized_party(CLIENT_ID)
        .build();
    assert!(validate(correct).is_ok());
}

#[test]
fn expired_not_yet_valid_and_future_issued_all_fail_closed() {
    let expired = VerifiedClaimsBuilder::new(ISSUER, "s", NOW - 61, NOW - 400)
        .audience(AUDIENCE)
        .nonce(NONCE)
        .build();
    assert_eq!(validate(expired), Err(ValidationError::Expired));

    let not_yet = VerifiedClaimsBuilder::new(ISSUER, "s", NOW + 500, NOW)
        .audience(AUDIENCE)
        .nonce(NONCE)
        .not_before(NOW + 200)
        .build();
    assert_eq!(validate(not_yet), Err(ValidationError::NotYetValid));

    let future = VerifiedClaimsBuilder::new(ISSUER, "s", NOW + 500, NOW + 200)
        .audience(AUDIENCE)
        .nonce(NONCE)
        .build();
    assert_eq!(validate(future), Err(ValidationError::IssuedInFuture));
}

#[test]
fn leeway_admits_a_marginally_skewed_token() {
    let within = VerifiedClaimsBuilder::new(ISSUER, "s", NOW - 30, NOW)
        .audience(AUDIENCE)
        .nonce(NONCE)
        .build();
    assert!(
        validate(within).is_ok(),
        "30s within 60s leeway is accepted"
    );
}

#[test]
fn missing_and_mismatched_nonce_fail_closed() {
    let no_nonce = VerifiedClaimsBuilder::new(ISSUER, "s", NOW + 300, NOW)
        .audience(AUDIENCE)
        .build();
    assert_eq!(validate(no_nonce), Err(ValidationError::MissingNonce));

    let wrong = VerifiedClaimsBuilder::new(ISSUER, "s", NOW + 300, NOW)
        .audience(AUDIENCE)
        .nonce("replayed-nonce")
        .build();
    assert_eq!(validate(wrong), Err(ValidationError::NonceMismatch));
}

#[test]
fn disallowed_algorithm_is_rejected() {
    let config = config();
    let validator = TokenValidator::new(&config);
    let verifier = FakeVerifier::proving(SigningAlgorithm::Es256, base_claims().build());
    assert_eq!(
        validator.validate(&verifier, &token(), NONCE, &FixedClock(NOW)),
        Err(ValidationError::DisallowedAlgorithm)
    );
}

#[test]
fn signature_failure_fails_closed() {
    let config = config();
    let validator = TokenValidator::new(&config);
    let verifier = FakeVerifier::failing(SignatureError::InvalidSignature);
    assert_eq!(
        validator.validate(&verifier, &token(), NONCE, &FixedClock(NOW)),
        Err(ValidationError::SignatureRejected)
    );
}

#[test]
fn empty_subject_is_rejected() {
    let claims = VerifiedClaimsBuilder::new(ISSUER, "", NOW + 300, NOW)
        .audience(AUDIENCE)
        .nonce(NONCE)
        .build();
    assert_eq!(validate(claims), Err(ValidationError::MissingSubject));
}

#[test]
fn retained_claims_expose_configured_role_claim_for_mapping() {
    let identity = validate(base_claims().string_claim("hakhub_role", "admin").build()).unwrap();
    let claim = ClaimName::from_str("hakhub_role").unwrap();
    assert_eq!(identity.claims().claim_values(&claim), ["admin"]);
}
