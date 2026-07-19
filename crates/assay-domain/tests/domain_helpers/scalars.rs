use std::str::FromStr;

use assay_domain::{
    AnalysisVersion, ContentHash, EvidenceId, EvidenceStatus, RepositorySource, RevisionId,
    RuleSetHash,
};

use super::{SHA256, local_source};

#[test]
fn validated_scalar_values_round_trip_as_strings() {
    for value in [
        serde_json::to_value(RevisionId::from_str(super::REVISION).unwrap()).unwrap(),
        serde_json::to_value(ContentHash::from_str(SHA256).unwrap()).unwrap(),
        serde_json::to_value(EvidenceId::from_str("evidence:readme:claim-4").unwrap()).unwrap(),
        serde_json::to_value(AnalysisVersion::from_str("project-intelligence-1").unwrap()).unwrap(),
        serde_json::to_value(RuleSetHash::from_str(SHA256).unwrap()).unwrap(),
    ] {
        assert!(value.is_string());
    }

    let revision: RevisionId = serde_json::from_str(&format!("\"{}\"", super::REVISION)).unwrap();
    let hash: ContentHash = serde_json::from_str(&format!("\"{SHA256}\"")).unwrap();
    let evidence_id: EvidenceId = serde_json::from_str("\"evidence:readme:claim-4\"").unwrap();
    let version: AnalysisVersion = serde_json::from_str("\"project-intelligence-1\"").unwrap();
    let rule_set_hash: RuleSetHash = serde_json::from_str(&format!("\"{SHA256}\"")).unwrap();

    assert_eq!(revision.as_str(), super::REVISION);
    assert_eq!(hash.as_str(), SHA256);
    assert_eq!(evidence_id.as_str(), "evidence:readme:claim-4");
    assert_eq!(version.as_str(), "project-intelligence-1");
    assert_eq!(rule_set_hash.as_str(), SHA256);
}

#[test]
fn scalar_values_reject_ambiguous_or_unsafe_input() {
    for invalid in ["HEAD", "main", "ABCDEF", "../revision"] {
        assert!(RevisionId::from_str(invalid).is_err(), "accepted {invalid}");
        assert!(serde_json::from_str::<RevisionId>(&format!("\"{invalid}\"")).is_err());
    }

    for invalid in [
        "0123456789abcdef0123456789abcdef0123456",
        "0123456789abcdef0123456789abcdef0123456g",
        "0123456789ABCDEF0123456789ABCDEF01234567",
        "0000000000000000000000000000000000000000",
        "0000000000000000000000000000000000000000000000000000000000000000",
    ] {
        assert!(RevisionId::from_str(invalid).is_err(), "accepted {invalid}");
        assert!(serde_json::from_str::<RevisionId>(&format!("\"{invalid}\"")).is_err());
    }

    for invalid in [
        "",
        "sha1:0123456789abcdef0123456789abcdef01234567",
        "sha256:short",
        "sha256:0123456789ABCDEF0123456789abcdef0123456789abcdef0123456789abcdef",
    ] {
        assert!(
            ContentHash::from_str(invalid).is_err(),
            "accepted {invalid}"
        );
        assert!(
            RuleSetHash::from_str(invalid).is_err(),
            "accepted {invalid}"
        );
    }

    for invalid in [
        "readme:claim-4",
        "evidence",
        "evidence::claim-4",
        "evidence:readme:/private/source.rs",
        "evidence:readme:Claim-4",
    ] {
        assert!(EvidenceId::from_str(invalid).is_err(), "accepted {invalid}");
    }

    for invalid in ["", "Project-Intelligence-1", "../analysis-1", "analysis 1"] {
        assert!(
            AnalysisVersion::from_str(invalid).is_err(),
            "accepted {invalid}"
        );
    }
}

#[test]
fn rule_set_hash_reports_its_own_value_kind_without_echoing_input() {
    let invalid = "sha256:private-input";

    let from_str_error = RuleSetHash::from_str(invalid).unwrap_err();
    let from_string_error = RuleSetHash::try_from(invalid.to_owned()).unwrap_err();

    for error in [from_str_error, from_string_error] {
        assert_eq!(error.value_kind(), "rule_set_hash");
        assert!(!error.to_string().contains(invalid));
    }
}

#[test]
fn repository_sources_are_portable_and_do_not_expose_local_paths() {
    let local = local_source();
    assert_eq!(local.local_repository_id().unwrap().as_str(), SHA256);
    assert!(local.hosted_locator().is_none());
    assert_eq!(
        serde_json::to_value(&local).unwrap(),
        serde_json::json!({"kind": "local", "repository_id": SHA256})
    );

    let hosted = RepositorySource::hosted("github", "assay-project", "assay").unwrap();
    assert_eq!(
        hosted.hosted_locator(),
        Some(("github", "assay-project", "assay"))
    );
    assert!(hosted.local_repository_id().is_none());
    assert_eq!(
        serde_json::to_value(&hosted).unwrap(),
        serde_json::json!({
            "kind": "hosted",
            "provider": "github",
            "namespace": "assay-project",
            "repository": "assay"
        })
    );

    for invalid in [
        serde_json::json!({"kind": "hosted", "provider": "https://github.com", "namespace": "assay", "repository": "assay"}),
        serde_json::json!({"kind": "hosted", "provider": "github", "namespace": "/private/user", "repository": "assay"}),
        serde_json::json!({"kind": "hosted", "provider": "github", "namespace": "assay", "repository": "../assay"}),
        serde_json::json!({"kind": "hosted", "provider": "GitHub", "namespace": "assay", "repository": "assay"}),
    ] {
        assert!(serde_json::from_value::<RepositorySource>(invalid).is_err());
    }
}

#[test]
fn statuses_have_stable_snake_case_names_and_reject_unknown_values() {
    let evidence_statuses = [
        (EvidenceStatus::Complete, "complete"),
        (EvidenceStatus::Partial, "partial"),
        (EvidenceStatus::Unavailable, "unavailable"),
        (EvidenceStatus::Unsupported, "unsupported"),
        (EvidenceStatus::Insufficient, "insufficient"),
        (EvidenceStatus::Pending, "pending"),
    ];
    let analysis_statuses = [
        (assay_domain::AnalysisStatus::Complete, "complete"),
        (assay_domain::AnalysisStatus::Partial, "partial"),
        (assay_domain::AnalysisStatus::Unavailable, "unavailable"),
        (assay_domain::AnalysisStatus::Unsupported, "unsupported"),
        (assay_domain::AnalysisStatus::Insufficient, "insufficient"),
        (assay_domain::AnalysisStatus::Pending, "pending"),
    ];

    for (status, name) in evidence_statuses {
        assert_eq!(
            serde_json::to_string(&status).unwrap(),
            format!("\"{name}\"")
        );
        assert_eq!(
            serde_json::from_str::<EvidenceStatus>(&format!("\"{name}\"")).unwrap(),
            status
        );
    }
    for (status, name) in analysis_statuses {
        assert_eq!(
            serde_json::to_string(&status).unwrap(),
            format!("\"{name}\"")
        );
        assert_eq!(
            serde_json::from_str::<assay_domain::AnalysisStatus>(&format!("\"{name}\"")).unwrap(),
            status
        );
    }

    for invalid in [
        "available",
        "insufficient_data",
        "pending_maturation",
        "unknown",
    ] {
        assert!(serde_json::from_str::<EvidenceStatus>(&format!("\"{invalid}\"")).is_err());
        assert!(
            serde_json::from_str::<assay_domain::AnalysisStatus>(&format!("\"{invalid}\""))
                .is_err()
        );
    }
}

#[test]
fn warning_and_limitation_codes_are_stable_safe_identifiers() {
    for invalid in [
        "",
        "HistoryUnavailable",
        "history-unavailable",
        "history unavailable",
        "/private/source",
        "token=secret",
    ] {
        assert!(
            assay_domain::Warning::new(invalid).is_err(),
            "accepted {invalid}"
        );
        assert!(
            assay_domain::Limitation::new(invalid).is_err(),
            "accepted {invalid}"
        );
    }

    assert!(serde_json::from_str::<assay_domain::Warning>(r#"{"code":"not snake case"}"#).is_err());
    assert!(serde_json::from_str::<assay_domain::Limitation>(r#"{"code":"../private"}"#).is_err());
}
