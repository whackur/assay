use std::cell::Cell;
use std::str::FromStr;

use assay_domain::{EvidenceStatus, RepositorySource};

use super::{FakeSearch, candidate, evidence, hosted, revision, seed};
use crate::comparison::cohort::discover_cohort;
use crate::comparison::policy::ComparisonPolicy;
use crate::comparison::types::{
    CandidateDescriptor, CandidateSearchOutcome, CohortMode, ComparisonErrorKind,
    ComparisonProfile, SeedProject,
};

#[test]
fn every_canonical_facet_is_reported_with_explicit_unavailability() {
    // The seed declares one facet; the contract still enumerates all four.
    let profile = ComparisonProfile::new(
        CohortMode::FunctionalCohort,
        vec![("problem_overlap".to_owned(), vec!["scoring".to_owned()])],
        vec![evidence("evidence:repository:snapshot")],
    )
    .unwrap();
    let narrow_seed = SeedProject::new(hosted("example-org", "seed"), revision(), profile);
    let search = FakeSearch {
        outcome: CandidateSearchOutcome::new(
            EvidenceStatus::Complete,
            vec![candidate(
                "a",
                Some(10),
                vec![("problem_overlap", vec!["scoring"])],
            )],
        )
        .unwrap(),
        calls: Cell::new(0),
    };
    let value = discover_cohort(&narrow_seed, &search, &ComparisonPolicy::v1())
        .unwrap()
        .to_machine_value();
    let facets = value["detailed_candidates"][0]["facets"]
        .as_array()
        .unwrap();
    let names = facets
        .iter()
        .map(|facet| facet["facet"].as_str().unwrap().to_owned())
        .collect::<Vec<_>>();
    assert_eq!(
        names,
        [
            "feature_overlap",
            "problem_overlap",
            "structural_similarity",
            "technical_similarity",
        ],
        "all four canonical facets must always be enumerated"
    );
    for facet in facets {
        if facet["facet"] == "problem_overlap" {
            assert_eq!(facet["status"], "complete");
        } else {
            assert_eq!(facet["status"], "unavailable");
            assert!(facet["value"].is_null(), "an absent facet is never a zero");
        }
    }
    assert_eq!(
        value["facet_weights"].as_array().unwrap().len(),
        4,
        "weights cover the canonical facet set"
    );
}

#[test]
fn a_non_canonical_profile_facet_is_rejected() {
    assert_eq!(
        ComparisonProfile::new(
            CohortMode::FunctionalCohort,
            vec![("custom_facet".to_owned(), vec!["token".to_owned()])],
            vec![evidence("evidence:repository:snapshot")],
        )
        .unwrap_err()
        .kind(),
        ComparisonErrorKind::NonCanonicalFacet
    );
    // Curated facets are canonical only for curated mode and vice versa.
    assert_eq!(
        ComparisonProfile::new(
            CohortMode::FunctionalCohort,
            vec![("entry_overlap".to_owned(), vec!["token".to_owned()])],
            vec![evidence("evidence:repository:snapshot")],
        )
        .unwrap_err()
        .kind(),
        ComparisonErrorKind::NonCanonicalFacet
    );
}

#[test]
fn curated_mode_enumerates_five_canonical_facets_including_maintenance_evidence() {
    // Specification 7.3 lists five curated comparison criteria; the fifth
    // is maintenance evidence.
    let profile = ComparisonProfile::new(
        CohortMode::CuratedList,
        vec![("entry_overlap".to_owned(), vec!["rust".to_owned()])],
        vec![evidence("evidence:repository:snapshot")],
    )
    .unwrap();
    let curated_seed = SeedProject::new(hosted("example-org", "awesome-seed"), revision(), profile);
    let curated_candidate = CandidateDescriptor::new(
        hosted("other-org", "awesome-rust"),
        revision(),
        true,
        vec![("entry_overlap".to_owned(), vec!["rust".to_owned()])],
        Some(100),
        evidence("evidence:github:candidate-awesome"),
    )
    .unwrap();
    let search = FakeSearch {
        outcome: CandidateSearchOutcome::new(EvidenceStatus::Complete, vec![curated_candidate])
            .unwrap(),
        calls: Cell::new(0),
    };
    let value = discover_cohort(&curated_seed, &search, &ComparisonPolicy::v1())
        .unwrap()
        .to_machine_value();
    let names = value["detailed_candidates"][0]["facets"]
        .as_array()
        .unwrap()
        .iter()
        .map(|facet| facet["facet"].as_str().unwrap().to_owned())
        .collect::<Vec<_>>();
    assert_eq!(
        names,
        [
            "editorial_quality",
            "entry_overlap",
            "list_structure",
            "maintenance_evidence",
            "unique_coverage",
        ],
        "curated comparisons enumerate all five specification criteria"
    );
    assert_eq!(value["facet_weights"].as_array().unwrap().len(), 5);
    // maintenance_evidence is a canonical profile facet, not an error.
    ComparisonProfile::new(
        CohortMode::CuratedList,
        vec![(
            "maintenance_evidence".to_owned(),
            vec!["recent_updates".to_owned()],
        )],
        vec![evidence("evidence:repository:snapshot")],
    )
    .unwrap();
}

#[test]
fn non_canonical_candidate_facet_tokens_never_reach_public_output() {
    let search = FakeSearch {
        outcome: CandidateSearchOutcome::new(
            EvidenceStatus::Complete,
            vec![candidate(
                "leaky",
                Some(5),
                vec![
                    ("problem_overlap", vec!["scoring"]),
                    ("custom_facet", vec!["leaked_token"]),
                ],
            )],
        )
        .unwrap(),
        calls: Cell::new(0),
    };
    let value = discover_cohort(&seed(), &search, &ComparisonPolicy::v1())
        .unwrap()
        .to_machine_value();
    let serialized = serde_json::to_string(&value).unwrap();
    assert!(
        !serialized.contains("leaked_token"),
        "a non-canonical candidate facet token must not reach public output"
    );
    assert!(
        !serialized.contains("custom_facet"),
        "a non-canonical candidate facet name must not reach public output"
    );
    assert_eq!(
        value["detailed_candidates"][0]["facets"]
            .as_array()
            .unwrap()
            .len(),
        4,
        "only canonical facets are enumerated"
    );
}

#[test]
fn a_local_candidate_is_rejected() {
    let local = RepositorySource::local(
        assay_domain::ContentHash::from_str(&format!("sha256:{}", "a".repeat(64))).unwrap(),
    );
    assert_eq!(
        CandidateDescriptor::new(
            local,
            revision(),
            false,
            vec![("problem_overlap".to_owned(), vec!["scoring".to_owned()])],
            None,
            evidence("evidence:github:candidate-local"),
        )
        .unwrap_err()
        .kind(),
        ComparisonErrorKind::CandidateNotHosted
    );
}
