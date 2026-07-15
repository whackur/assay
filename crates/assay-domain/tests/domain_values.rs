use std::str::FromStr;

use assay_domain::{
    AnalysisManifest, AnalysisStatus, AnalysisVersion, ContentHash, EvidenceId, EvidenceSource,
    EvidenceSourceKind, EvidenceStatus, Limitation, RepositorySource, RevisionId, RuleSetHash,
    SourceSnapshot, Warning,
};

const REVISION: &str = "0123456789abcdef0123456789abcdef01234567";
const TREE: &str = "89abcdef0123456789abcdef0123456789abcdef";
const SHA256: &str = "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
const OTHER_SHA256: &str =
    "sha256:abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789";

fn local_source() -> RepositorySource {
    RepositorySource::local(ContentHash::from_str(SHA256).unwrap())
}

fn snapshot() -> SourceSnapshot {
    SourceSnapshot::new(
        local_source(),
        RevisionId::from_str(REVISION).unwrap(),
        Some(RevisionId::from_str(TREE).unwrap()),
    )
}

fn evidence(status: EvidenceStatus) -> EvidenceSource {
    EvidenceSource::at_revision(
        EvidenceId::from_str("evidence:repository:snapshot").unwrap(),
        EvidenceSourceKind::Repository,
        status,
        RevisionId::from_str(REVISION).unwrap(),
    )
}

#[test]
fn validated_scalar_values_round_trip_as_strings() {
    for value in [
        serde_json::to_value(RevisionId::from_str(REVISION).unwrap()).unwrap(),
        serde_json::to_value(ContentHash::from_str(SHA256).unwrap()).unwrap(),
        serde_json::to_value(EvidenceId::from_str("evidence:readme:claim-4").unwrap()).unwrap(),
        serde_json::to_value(AnalysisVersion::from_str("project-intelligence-1").unwrap()).unwrap(),
        serde_json::to_value(RuleSetHash::from_str(SHA256).unwrap()).unwrap(),
    ] {
        assert!(value.is_string());
    }

    let revision: RevisionId = serde_json::from_str(&format!("\"{REVISION}\"")).unwrap();
    let hash: ContentHash = serde_json::from_str(&format!("\"{SHA256}\"")).unwrap();
    let evidence_id: EvidenceId = serde_json::from_str("\"evidence:readme:claim-4\"").unwrap();
    let version: AnalysisVersion = serde_json::from_str("\"project-intelligence-1\"").unwrap();
    let rule_set_hash: RuleSetHash = serde_json::from_str(&format!("\"{SHA256}\"")).unwrap();

    assert_eq!(revision.as_str(), REVISION);
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
    ] {
        assert!(RevisionId::from_str(invalid).is_err(), "accepted {invalid}");
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
        (AnalysisStatus::Complete, "complete"),
        (AnalysisStatus::Partial, "partial"),
        (AnalysisStatus::Unavailable, "unavailable"),
        (AnalysisStatus::Unsupported, "unsupported"),
        (AnalysisStatus::Insufficient, "insufficient"),
        (AnalysisStatus::Pending, "pending"),
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
            serde_json::from_str::<AnalysisStatus>(&format!("\"{name}\"")).unwrap(),
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
        assert!(serde_json::from_str::<AnalysisStatus>(&format!("\"{invalid}\"")).is_err());
    }
}

#[test]
fn evidence_sources_require_provenance_for_complete_or_partial_evidence() {
    for status in [EvidenceStatus::Complete, EvidenceStatus::Partial] {
        assert!(
            EvidenceSource::unresolved(
                EvidenceId::from_str("evidence:repository:history").unwrap(),
                EvidenceSourceKind::RepositoryHistory,
                status,
            )
            .is_err()
        );
    }

    for status in [
        EvidenceStatus::Unavailable,
        EvidenceStatus::Unsupported,
        EvidenceStatus::Insufficient,
        EvidenceStatus::Pending,
    ] {
        EvidenceSource::unresolved(
            EvidenceId::from_str("evidence:repository:history").unwrap(),
            EvidenceSourceKind::RepositoryHistory,
            status,
        )
        .unwrap();
    }

    let invalid = serde_json::json!({
        "id": "evidence:repository:history",
        "kind": "repository_history",
        "status": "complete",
        "revision": null,
        "content_hash": null
    });
    assert!(serde_json::from_value::<EvidenceSource>(invalid).is_err());

    let pinned = evidence(EvidenceStatus::Complete);
    assert_eq!(pinned.kind(), EvidenceSourceKind::Repository);
    assert_eq!(pinned.status(), EvidenceStatus::Complete);
    assert_eq!(pinned.revision().unwrap().as_str(), REVISION);
    assert!(pinned.content_hash().is_none());
}

#[test]
fn manifest_round_trip_preserves_raw_evidence_and_derived_analysis_state() {
    let unavailable_history = EvidenceSource::unresolved(
        EvidenceId::from_str("evidence:repository:history").unwrap(),
        EvidenceSourceKind::RepositoryHistory,
        EvidenceStatus::Unavailable,
    )
    .unwrap();
    let content = EvidenceSource::at_content(
        EvidenceId::from_str("evidence:repository:readme").unwrap(),
        EvidenceSourceKind::RepositoryContent,
        EvidenceStatus::Complete,
        RevisionId::from_str(REVISION).unwrap(),
        ContentHash::from_str(OTHER_SHA256).unwrap(),
    );
    let manifest = AnalysisManifest::new(
        snapshot(),
        AnalysisVersion::from_str("analysis-1").unwrap(),
        RuleSetHash::from_str(SHA256).unwrap(),
        AnalysisStatus::Partial,
        vec![unavailable_history, content],
        vec![Warning::new("history_unavailable").unwrap()],
        vec![Limitation::new("durability_not_computed").unwrap()],
    )
    .unwrap();

    let json = serde_json::to_string(&manifest).unwrap();
    let decoded: AnalysisManifest = serde_json::from_str(&json).unwrap();

    assert_eq!(decoded, manifest);
    assert_eq!(decoded.status(), AnalysisStatus::Partial);
    assert_eq!(decoded.source_snapshot().revision().as_str(), REVISION);
    assert_eq!(decoded.analysis_version().as_str(), "analysis-1");
    assert_eq!(decoded.rule_set_hash().as_str(), SHA256);
    assert_eq!(decoded.evidence_sources().len(), 2);
    assert_eq!(decoded.warnings()[0].code(), "history_unavailable");
    assert_eq!(decoded.limitations()[0].code(), "durability_not_computed");
    assert!(json.contains("\"status\":\"unavailable\""));
    assert!(!json.contains("/private/"));
    assert!(!json.contains("raw_diff"));
}

#[test]
fn manifest_canonicalizes_collection_order_and_rejects_duplicate_identifiers() {
    let manifest = AnalysisManifest::new(
        snapshot(),
        AnalysisVersion::from_str("analysis-1").unwrap(),
        RuleSetHash::from_str(SHA256).unwrap(),
        AnalysisStatus::Complete,
        vec![
            EvidenceSource::at_revision(
                EvidenceId::from_str("evidence:zeta:item").unwrap(),
                EvidenceSourceKind::Repository,
                EvidenceStatus::Complete,
                RevisionId::from_str(REVISION).unwrap(),
            ),
            EvidenceSource::at_revision(
                EvidenceId::from_str("evidence:alpha:item").unwrap(),
                EvidenceSourceKind::Repository,
                EvidenceStatus::Complete,
                RevisionId::from_str(REVISION).unwrap(),
            ),
        ],
        vec![
            Warning::new("zeta_warning").unwrap(),
            Warning::new("alpha_warning").unwrap(),
        ],
        vec![
            Limitation::new("zeta_limit").unwrap(),
            Limitation::new("alpha_limit").unwrap(),
        ],
    )
    .unwrap();
    let value = serde_json::to_value(manifest).unwrap();

    assert_eq!(value["evidence_sources"][0]["id"], "evidence:alpha:item");
    assert_eq!(value["warnings"][0]["code"], "alpha_warning");
    assert_eq!(value["limitations"][0]["code"], "alpha_limit");

    let duplicate_evidence = serde_json::json!({
        "source_snapshot": snapshot(),
        "analysis_version": "analysis-1",
        "rule_set_hash": SHA256,
        "status": "complete",
        "evidence_sources": [evidence(EvidenceStatus::Complete), evidence(EvidenceStatus::Complete)],
        "warnings": [],
        "limitations": []
    });
    assert!(serde_json::from_value::<AnalysisManifest>(duplicate_evidence).is_err());

    let empty_evidence = serde_json::json!({
        "source_snapshot": snapshot(),
        "analysis_version": "analysis-1",
        "rule_set_hash": SHA256,
        "status": "unavailable",
        "evidence_sources": [],
        "warnings": [{"code": "source_unavailable"}],
        "limitations": []
    });
    assert!(serde_json::from_value::<AnalysisManifest>(empty_evidence).is_err());

    let duplicate_warnings = serde_json::json!({
        "source_snapshot": snapshot(),
        "analysis_version": "analysis-1",
        "rule_set_hash": SHA256,
        "status": "partial",
        "evidence_sources": [evidence(EvidenceStatus::Partial)],
        "warnings": [{"code": "history_partial"}, {"code": "history_partial"}],
        "limitations": []
    });
    assert!(serde_json::from_value::<AnalysisManifest>(duplicate_warnings).is_err());
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
        assert!(Warning::new(invalid).is_err(), "accepted {invalid}");
        assert!(Limitation::new(invalid).is_err(), "accepted {invalid}");
    }

    assert!(serde_json::from_str::<Warning>(r#"{"code":"not snake case"}"#).is_err());
    assert!(serde_json::from_str::<Limitation>(r#"{"code":"../private"}"#).is_err());
}
