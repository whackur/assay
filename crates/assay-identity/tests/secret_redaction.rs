mod common;

use std::str::FromStr;

use assay_identity::{
    AccountKey, AuthorizationStore, CallbackParams, IssuerUrl, RedirectUri, Session, Subject,
    UnixTime, UpstreamIdToken,
};
use common::{ISSUER, REDIRECT, SeqEntropy, config};

const SENTINEL: &str = "SUPER-SECRET-TOKEN-abcdef0123456789";

#[test]
fn the_upstream_token_is_redacted_in_debug_and_only_revealed_deliberately() {
    let token = UpstreamIdToken::new(SENTINEL.to_owned());
    assert_eq!(token.reveal(), SENTINEL);
    let rendered = format!("{token:?}");
    assert!(!rendered.contains(SENTINEL));
    assert_eq!(rendered, "UpstreamIdToken(<redacted>)");
}

#[test]
fn the_pkce_verifier_and_nonce_are_redacted_in_a_verified_callback() {
    let config = config();
    let entropy = SeqEntropy::new(20);
    let mut store = AuthorizationStore::new();
    let redirect = store
        .begin(&config, &RedirectUri::from_str(REDIRECT).unwrap(), &entropy)
        .unwrap();
    let params = CallbackParams::new(redirect.state(), "auth-code-secret", REDIRECT);
    let verified = store.redeem(&params).unwrap();

    let verifier_value = verified.pkce_verifier().reveal().to_owned();
    let nonce_value = verified.nonce().reveal().to_owned();
    let rendered = format!("{verified:?}");

    assert!(!rendered.contains(&verifier_value));
    assert!(!rendered.contains(&nonce_value));
    assert!(!rendered.contains("auth-code-secret"));
    assert_eq!(
        format!("{:?}", verified.pkce_verifier()),
        "PkceVerifier(<redacted>)"
    );
    assert_eq!(format!("{:?}", verified.nonce()), "Nonce(<redacted>)");
}

#[test]
fn the_session_secret_is_redacted_in_debug() {
    let account = AccountKey::new(
        IssuerUrl::from_str(ISSUER).unwrap(),
        Subject::from_str("subject-123").unwrap(),
    );
    let entropy = SeqEntropy::new(21);
    let session = Session::establish(account, UnixTime::from_seconds(0), 3_600, &entropy);
    let secret = session.secret().reveal().to_owned();
    assert!(!format!("{session:?}").contains(&secret));
    assert_eq!(
        format!("{:?}", session.secret()),
        "SessionSecret(<redacted>)"
    );
}
