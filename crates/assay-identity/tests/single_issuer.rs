mod common;

use std::str::FromStr;

use assay_identity::{
    Audience, ClientId, IssuerUrl, OidcDeploymentConfigBuilder, RedirectUri, SigningAlgorithm,
    TokenValidator, UpstreamIdToken, ValidationError, VerifiedClaimsBuilder,
};
use common::{AUDIENCE, CLIENT_ID, FakeVerifier, FixedClock, ISSUER, REDIRECT};

const NOW: i64 = 1_000_000;

fn single_issuer_config() -> assay_identity::OidcDeploymentConfig {
    OidcDeploymentConfigBuilder::new(
        IssuerUrl::from_str(ISSUER).unwrap(),
        ClientId::from_str(CLIENT_ID).unwrap(),
        Audience::from_str(AUDIENCE).unwrap(),
    )
    .allow_redirect(RedirectUri::from_str(REDIRECT).unwrap())
    .allow_algorithm(SigningAlgorithm::Rs256)
    .single_issuer_no_local_registration(true)
    .build()
    .unwrap()
}

#[test]
fn a_single_issuer_deployment_offers_no_independent_registration() {
    let config = single_issuer_config();
    assert!(!config.allows_local_registration());
}

#[test]
fn a_token_for_the_upstream_applications_own_audience_is_rejected() {
    let config = single_issuer_config();
    let validator = TokenValidator::new(&config);
    let upstream_only = VerifiedClaimsBuilder::new(ISSUER, "subject-123", NOW + 300, NOW)
        .audience("hakhub-web")
        .nonce("n")
        .build();
    let verifier = FakeVerifier::proving(SigningAlgorithm::Rs256, upstream_only);
    let token = UpstreamIdToken::new("h.p.s".to_owned());
    assert_eq!(
        validator.validate(&verifier, &token, "n", &FixedClock(NOW)),
        Err(ValidationError::AudienceMismatch)
    );
}
