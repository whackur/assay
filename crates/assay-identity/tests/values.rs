use std::str::FromStr;

use assay_identity::{AccountKey, Audience, ClaimName, IssuerUrl, RedirectUri, Subject};

#[test]
fn issuer_url_requires_https_and_a_clean_authority() {
    assert!(IssuerUrl::from_str("https://issuer.example").is_ok());
    assert!(IssuerUrl::from_str("https://issuer.example/realms/assay").is_ok());
    assert!(IssuerUrl::from_str("http://issuer.example").is_err());
    assert!(IssuerUrl::from_str("https://user:pass@issuer.example").is_err());
    assert!(IssuerUrl::from_str("https://issuer.example#frag").is_err());
    assert!(IssuerUrl::from_str("https://issuer.example?q=1").is_err());
    assert!(IssuerUrl::from_str("https://").is_err());
    assert!(IssuerUrl::from_str("issuer.example").is_err());
}

#[test]
fn a_rejected_value_never_appears_in_the_error() {
    let error = IssuerUrl::from_str("http://secret-internal-host").unwrap_err();
    let rendered = format!("{error}");
    assert!(!rendered.contains("secret-internal-host"));
    assert_eq!(error.value_kind(), "issuer_url");
}

#[test]
fn redirect_uri_allows_a_query_but_forbids_a_fragment() {
    assert!(RedirectUri::from_str("https://assay.example/cb?x=1").is_ok());
    assert!(RedirectUri::from_str("https://assay.example/cb#frag").is_err());
}

#[test]
fn subject_and_audience_reject_control_characters() {
    assert!(Subject::from_str("sub-123").is_ok());
    assert!(Subject::from_str("").is_err());
    assert!(Subject::from_str("bad\nsubject").is_err());
    assert!(Audience::from_str("assay-api").is_ok());
    assert!(Audience::from_str("bad\taud").is_err());
}

#[test]
fn claim_name_accepts_safe_names_only() {
    assert!(ClaimName::from_str("hakhub_role").is_ok());
    assert!(ClaimName::from_str("groups").is_ok());
    assert!(ClaimName::from_str("bad name").is_err());
    assert!(ClaimName::from_str("").is_err());
}

#[test]
fn account_key_round_trips_through_serde() {
    let key = AccountKey::new(
        IssuerUrl::from_str("https://issuer.example").unwrap(),
        Subject::from_str("subject-123").unwrap(),
    );
    let json = serde_json::to_string(&key).unwrap();
    let restored: AccountKey = serde_json::from_str(&json).unwrap();
    assert_eq!(key, restored);
    assert!(json.contains("https://issuer.example"));
    assert!(json.contains("subject-123"));
}

#[test]
fn account_key_deserialization_rejects_unknown_fields() {
    let json = r#"{"issuer":"https://issuer.example","subject":"s","email":"x@y.z"}"#;
    assert!(serde_json::from_str::<AccountKey>(json).is_err());
}
