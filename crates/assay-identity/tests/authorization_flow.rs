mod common;

use std::str::FromStr;

use assay_identity::{AuthorizationStore, CallbackParams, FlowError, RedirectUri};
use common::{REDIRECT, SeqEntropy, config, pkce_s256};

fn redirect() -> RedirectUri {
    RedirectUri::from_str(REDIRECT).unwrap()
}

#[test]
fn begin_emits_state_nonce_and_s256_challenge_without_any_upstream_token() {
    let config = config();
    let entropy = SeqEntropy::new(1);
    let mut store = AuthorizationStore::new();
    let redirect = store.begin(&config, &redirect(), &entropy).unwrap();

    assert!(!redirect.state().is_empty());
    assert!(!redirect.nonce().is_empty());
    assert_ne!(redirect.state(), redirect.nonce());
    assert_eq!(redirect.code_challenge_method(), "S256");
    assert!(!redirect.code_challenge().as_str().is_empty());
    assert_eq!(redirect.redirect_uri().as_str(), REDIRECT);
    assert_eq!(store.pending_count(), 1);
}

#[test]
fn redeem_binds_verifier_to_the_challenge_and_returns_the_nonce() {
    let config = config();
    let entropy = SeqEntropy::new(2);
    let mut store = AuthorizationStore::new();
    let redirect = store.begin(&config, &redirect(), &entropy).unwrap();
    let challenge = redirect.code_challenge().as_str().to_owned();
    let nonce = redirect.nonce().to_owned();

    let params = CallbackParams::new(redirect.state(), "auth-code", REDIRECT);
    let verified = store.redeem(&params).unwrap();

    assert_eq!(verified.code(), "auth-code");
    assert_eq!(verified.nonce().reveal(), nonce);
    assert_eq!(pkce_s256(verified.pkce_verifier().reveal()), challenge);
    assert_eq!(store.pending_count(), 0);
}

#[test]
fn a_replayed_callback_fails_closed() {
    let config = config();
    let entropy = SeqEntropy::new(3);
    let mut store = AuthorizationStore::new();
    let redirect = store.begin(&config, &redirect(), &entropy).unwrap();
    let params = CallbackParams::new(redirect.state(), "auth-code", REDIRECT);

    assert!(store.redeem(&params).is_ok());
    assert_eq!(
        store.redeem(&params).err(),
        Some(FlowError::UnknownOrReplayedState),
        "single-use state cannot be replayed"
    );
}

#[test]
fn an_unknown_state_fails_closed() {
    let mut store = AuthorizationStore::new();
    let params = CallbackParams::new("forged-state", "auth-code", REDIRECT);
    assert_eq!(
        store.redeem(&params).err(),
        Some(FlowError::UnknownOrReplayedState)
    );
}

#[test]
fn a_mismatched_redirect_uri_fails_closed() {
    let config = config();
    let entropy = SeqEntropy::new(4);
    let mut store = AuthorizationStore::new();
    let redirect = store.begin(&config, &redirect(), &entropy).unwrap();
    let params = CallbackParams::new(
        redirect.state(),
        "auth-code",
        "https://assay.example/elsewhere",
    );
    assert_eq!(
        store.redeem(&params).err(),
        Some(FlowError::RedirectMismatch)
    );
}

#[test]
fn a_redirect_outside_the_allowlist_is_refused_at_begin() {
    let config = config();
    let entropy = SeqEntropy::new(5);
    let mut store = AuthorizationStore::new();
    let outside = RedirectUri::from_str("https://attacker.example/callback").unwrap();
    assert_eq!(
        store.begin(&config, &outside, &entropy).err(),
        Some(FlowError::RedirectNotAllowed)
    );
}

#[test]
fn an_empty_code_fails_closed() {
    let config = config();
    let entropy = SeqEntropy::new(6);
    let mut store = AuthorizationStore::new();
    let redirect = store.begin(&config, &redirect(), &entropy).unwrap();
    let params = CallbackParams::new(redirect.state(), "", REDIRECT);
    assert_eq!(store.redeem(&params).err(), Some(FlowError::EmptyCode));
}

#[test]
fn independent_transactions_use_distinct_secrets() {
    let config = config();
    let entropy = SeqEntropy::new(7);
    let mut store = AuthorizationStore::new();
    let first = store.begin(&config, &redirect(), &entropy).unwrap();
    let second = store.begin(&config, &redirect(), &entropy).unwrap();
    assert_ne!(first.state(), second.state());
    assert_ne!(first.nonce(), second.nonce());
    assert_ne!(
        first.code_challenge().as_str(),
        second.code_challenge().as_str()
    );
}
