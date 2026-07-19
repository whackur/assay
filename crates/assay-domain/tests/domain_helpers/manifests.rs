use std::str::FromStr;

use assay_domain::{
    AnalysisManifest, AnalysisStatus, AnalysisVersion, ContentHash, EvidenceId, EvidenceSource,
    EvidenceSourceKind, EvidenceStatus, Limitation, RevisionId, RuleSetHash, Warning,
};

use super::{OTHER_SHA256, REVISION, SHA256, evidence, snapshot};

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
fn complete_manifest_rejects_every_non_complete_evidence_status() {
    for evidence_status in [
        EvidenceStatus::Partial,
        EvidenceStatus::Unavailable,
        EvidenceStatus::Unsupported,
        EvidenceStatus::Insufficient,
        EvidenceStatus::Pending,
    ] {
        let direct = AnalysisManifest::new(
            snapshot(),
            AnalysisVersion::from_str("analysis-1").unwrap(),
            RuleSetHash::from_str(SHA256).unwrap(),
            AnalysisStatus::Complete,
            vec![evidence(evidence_status)],
            vec![],
            vec![],
        );
        assert!(direct.is_err(), "accepted direct {evidence_status:?}");

        let serialized = serde_json::json!({
            "source_snapshot": snapshot(),
            "analysis_version": "analysis-1",
            "rule_set_hash": SHA256,
            "status": "complete",
            "evidence_sources": [evidence(evidence_status)],
            "warnings": [],
            "limitations": []
        });
        assert!(
            serde_json::from_value::<AnalysisManifest>(serialized).is_err(),
            "accepted serialized {evidence_status:?}"
        );
    }
}
