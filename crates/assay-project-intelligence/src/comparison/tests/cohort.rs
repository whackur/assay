use std::cell::Cell;

use assay_domain::EvidenceStatus;

use super::{FakeSearch, candidate, evidence, hosted, revision, seed};
use crate::comparison::cohort::discover_cohort;
use crate::comparison::policy::ComparisonPolicy;
use crate::comparison::types::{
    CandidateDescriptor, CandidateSearchOutcome, CohortMode, ComparisonProfile, SeedProject,
};

#[test]
fn the_search_port_is_invoked_exactly_once() {
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
    discover_cohort(&seed(), &search, &ComparisonPolicy::v1()).unwrap();
    assert_eq!(search.calls.get(), 1, "discovery must stop at one depth");
}

#[test]
fn popularity_orders_ties_but_never_changes_similarity() {
    // Two candidates with identical tokens but different star counts.
    let facets = vec![
        ("problem_overlap", vec!["dependency_analysis", "scoring"]),
        ("feature_overlap", vec!["cli", "json_output"]),
    ];
    let search = FakeSearch {
        outcome: CandidateSearchOutcome::new(
            EvidenceStatus::Complete,
            vec![
                candidate("low-stars", Some(1), facets.clone()),
                candidate("high-stars", Some(9_000), facets.clone()),
            ],
        )
        .unwrap(),
        calls: Cell::new(0),
    };
    let comparison = discover_cohort(&seed(), &search, &ComparisonPolicy::v1()).unwrap();
    let detailed = comparison.detailed_candidates();
    assert_eq!(detailed.len(), 2);
    assert_eq!(
        detailed[0].overall_similarity(),
        detailed[1].overall_similarity(),
        "identical tokens must produce identical similarity regardless of stars"
    );
    assert_eq!(
        detailed[0].identifier(),
        "github/other-org/high-stars",
        "stars break the ordering tie only"
    );
    assert_eq!(detailed[0].overall_similarity(), Some(1.0));
}

#[test]
fn only_five_candidates_are_detailed_and_the_rest_are_compact() {
    let mut candidates = Vec::new();
    for index in 0..7 {
        candidates.push(candidate(
            &format!("repo{index}"),
            Some(index),
            vec![("problem_overlap", vec!["scoring"])],
        ));
    }
    let search = FakeSearch {
        outcome: CandidateSearchOutcome::new(EvidenceStatus::Complete, candidates).unwrap(),
        calls: Cell::new(0),
    };
    let comparison = discover_cohort(&seed(), &search, &ComparisonPolicy::v1()).unwrap();
    assert_eq!(comparison.detailed_candidates().len(), 5);
    assert_eq!(comparison.additional_candidates().len(), 2);
}

#[test]
fn a_candidate_with_no_shared_facet_is_insufficient_not_zero() {
    let search = FakeSearch {
        outcome: CandidateSearchOutcome::new(
            EvidenceStatus::Complete,
            vec![candidate(
                "unrelated",
                Some(5),
                vec![("technical_similarity", vec!["rust"])],
            )],
        )
        .unwrap(),
        calls: Cell::new(0),
    };
    let comparison = discover_cohort(&seed(), &search, &ComparisonPolicy::v1()).unwrap();
    assert!(comparison.detailed_candidates().is_empty());
    assert_eq!(comparison.status(), EvidenceStatus::Insufficient);
}

#[test]
fn an_unavailable_search_stays_unavailable_without_fabricated_candidates() {
    let search = FakeSearch {
        outcome: CandidateSearchOutcome::new(EvidenceStatus::Unavailable, Vec::new()).unwrap(),
        calls: Cell::new(0),
    };
    let comparison = discover_cohort(&seed(), &search, &ComparisonPolicy::v1()).unwrap();
    assert_eq!(comparison.status(), EvidenceStatus::Unavailable);
    assert!(comparison.detailed_candidates().is_empty());
}

#[test]
fn curated_mode_excludes_non_curated_candidates() {
    let curated_profile = ComparisonProfile::new(
        CohortMode::CuratedList,
        vec![(
            "entry_overlap".to_owned(),
            vec!["rust".to_owned(), "wasm".to_owned()],
        )],
        vec![evidence("evidence:repository:snapshot")],
    )
    .unwrap();
    let curated_seed = SeedProject::new(
        hosted("example-org", "awesome-seed"),
        revision(),
        curated_profile,
    );
    let curated_candidate = CandidateDescriptor::new(
        hosted("other-org", "awesome-rust"),
        revision(),
        true,
        vec![("entry_overlap".to_owned(), vec!["rust".to_owned()])],
        Some(100),
        evidence("evidence:github:candidate-awesome"),
    )
    .unwrap();
    let library_candidate = candidate(
        "a-library",
        Some(500),
        vec![("entry_overlap", vec!["rust"])],
    );
    let search = FakeSearch {
        outcome: CandidateSearchOutcome::new(
            EvidenceStatus::Complete,
            vec![curated_candidate, library_candidate],
        )
        .unwrap(),
        calls: Cell::new(0),
    };
    let comparison = discover_cohort(&curated_seed, &search, &ComparisonPolicy::v1()).unwrap();
    assert_eq!(comparison.detailed_candidates().len(), 1);
    assert_eq!(
        comparison.detailed_candidates()[0].identifier(),
        "github/other-org/awesome-rust"
    );
    assert_eq!(comparison.status(), EvidenceStatus::Partial);
}

#[test]
fn differentiators_separate_seed_only_and_candidate_only_tokens() {
    let search = FakeSearch {
        outcome: CandidateSearchOutcome::new(
            EvidenceStatus::Complete,
            vec![candidate(
                "partial-overlap",
                Some(5),
                vec![
                    ("problem_overlap", vec!["scoring"]),
                    ("feature_overlap", vec!["cli", "web_dashboard"]),
                ],
            )],
        )
        .unwrap(),
        calls: Cell::new(0),
    };
    let comparison = discover_cohort(&seed(), &search, &ComparisonPolicy::v1()).unwrap();
    let value = comparison.to_machine_value();
    let differentiators = &value["detailed_candidates"][0]["differentiators"];
    let seed_only = differentiators["seed_only"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item["token"].as_str().unwrap().to_owned())
        .collect::<Vec<_>>();
    let candidate_only = differentiators["candidate_only"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item["token"].as_str().unwrap().to_owned())
        .collect::<Vec<_>>();
    assert!(seed_only.contains(&"dependency_analysis".to_owned()));
    assert!(seed_only.contains(&"json_output".to_owned()));
    assert_eq!(candidate_only, vec!["web_dashboard".to_owned()]);
}

#[test]
fn discovery_is_byte_deterministic() {
    let build = || {
        let search = FakeSearch {
            outcome: CandidateSearchOutcome::new(
                EvidenceStatus::Complete,
                vec![
                    candidate("a", Some(3), vec![("problem_overlap", vec!["scoring"])]),
                    candidate("b", Some(3), vec![("feature_overlap", vec!["cli"])]),
                ],
            )
            .unwrap(),
            calls: Cell::new(0),
        };
        serde_json::to_vec(
            &discover_cohort(&seed(), &search, &ComparisonPolicy::v1())
                .unwrap()
                .to_machine_value(),
        )
        .unwrap()
    };
    assert_eq!(build(), build());
}

#[test]
fn a_zero_overlap_candidate_is_demoted_rather_than_detailed_without_reasons() {
    // A shared facet with fully disjoint tokens yields no selection reason.
    let search = FakeSearch {
        outcome: CandidateSearchOutcome::new(
            EvidenceStatus::Complete,
            vec![candidate(
                "disjoint",
                Some(9_000),
                vec![("problem_overlap", vec!["image_processing"])],
            )],
        )
        .unwrap(),
        calls: Cell::new(0),
    };
    let comparison = discover_cohort(&seed(), &search, &ComparisonPolicy::v1()).unwrap();
    assert!(
        comparison.detailed_candidates().is_empty(),
        "a candidate without a cited selection reason must not be detailed"
    );
    assert_eq!(comparison.status(), EvidenceStatus::Insufficient);
    let value = comparison.to_machine_value();
    assert_eq!(
        value["limitations"][0]["code"],
        "candidate_similarity_insufficient"
    );
}
