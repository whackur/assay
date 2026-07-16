mod common;

use std::str::FromStr;

use assay_identity::{
    AccountKey, IssuerUrl, Session, SessionError, SessionState, Subject, UnixTime,
};
use common::{ISSUER, SeqEntropy};

const START: i64 = 2_000_000;
const TTL: i64 = 3_600;

fn account() -> AccountKey {
    AccountKey::new(
        IssuerUrl::from_str(ISSUER).unwrap(),
        Subject::from_str("subject-123").unwrap(),
    )
}

fn session(entropy: &SeqEntropy) -> Session {
    Session::establish(account(), UnixTime::from_seconds(START), TTL, entropy)
}

#[test]
fn an_established_session_is_active_until_expiry() {
    let entropy = SeqEntropy::new(10);
    let session = session(&entropy);
    assert_eq!(session.state(), SessionState::Active);
    assert!(session.is_valid(UnixTime::from_seconds(START + TTL - 1)));
    assert!(!session.is_valid(UnixTime::from_seconds(START + TTL)));
    assert_eq!(session.rotation_count(), 0);
}

#[test]
fn rotation_replaces_the_secret_and_keeps_the_session_identity() {
    let entropy = SeqEntropy::new(11);
    let session = session(&entropy);
    let before = session.secret().reveal().to_owned();
    let rotated = session
        .rotate(UnixTime::from_seconds(START + 10), TTL, &entropy)
        .unwrap();

    assert_ne!(rotated.secret().reveal(), before);
    assert_eq!(rotated.id(), session.id());
    assert_eq!(rotated.rotation_count(), 1);
    assert_eq!(rotated.expires_at(), START + 10 + TTL);
    assert!(rotated.is_valid(UnixTime::from_seconds(START + 10)));
}

#[test]
fn an_expired_session_cannot_rotate() {
    let entropy = SeqEntropy::new(12);
    let session = session(&entropy);
    assert_eq!(
        session
            .rotate(UnixTime::from_seconds(START + TTL), TTL, &entropy)
            .err(),
        Some(SessionError::Expired)
    );
}

#[test]
fn a_revoked_session_is_invalid_and_cannot_rotate() {
    let entropy = SeqEntropy::new(13);
    let mut session = session(&entropy);
    session.revoke();
    assert_eq!(session.state(), SessionState::Revoked);
    assert!(!session.is_valid(UnixTime::from_seconds(START)));
    assert_eq!(
        session
            .rotate(UnixTime::from_seconds(START + 1), TTL, &entropy)
            .err(),
        Some(SessionError::NotActive)
    );
}

#[test]
fn the_session_never_carries_upstream_token_material() {
    let entropy = SeqEntropy::new(14);
    let session = session(&entropy);
    let rendered = format!("{session:?}");
    assert!(
        !rendered.contains(session.secret().reveal()),
        "the opaque cookie secret must not appear in Debug"
    );
    assert!(rendered.contains("<redacted>"));
}
