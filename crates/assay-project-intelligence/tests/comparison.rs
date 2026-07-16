use std::{cell::Cell, path::PathBuf, str::FromStr};

use assay_domain::{EvidenceId, EvidenceStatus, RepositorySource, RevisionId};
use assay_project_intelligence::{
    CandidateDescriptor, CandidateSearch, CandidateSearchError, CandidateSearchOutcome, CohortMode,
    CohortQuery, ComparisonPolicy, ComparisonProfile, SeedProject, discover_cohort,
};
use jsonschema::{Draft, Validator};
use serde_json::Value;

fn evidence(value: &str) -> EvidenceId {
    EvidenceId::from_str(value).unwrap()
}

fn revision(value: &str) -> RevisionId {
    RevisionId::from_str(value).unwrap()
}

fn hosted(namespace: &str, repository: &str) -> RepositorySource {
    RepositorySource::hosted("github", namespace, repository).unwrap()
}

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|path| path.parent())
        .expect("crate must remain under crates/")
        .to_path_buf()
}

fn comparison_schema() -> Validator {
    let schema: Value = serde_json::from_str(
        &std::fs::read_to_string(repository_root().join("schemas/project-comparison/v1.json"))
            .expect("comparison schema must be readable"),
    )
    .expect("comparison schema must parse");
    jsonschema::options()
        .with_draft(Draft::Draft202012)
        .should_validate_formats(true)
        .build(&schema)
        .expect("comparison schema must build")
}

fn assert_schema_valid(value: &Value) {
    let errors = comparison_schema()
        .iter_errors(value)
        .map(|error| error.to_string())
        .collect::<Vec<_>>();
    assert!(
        errors.is_empty(),
        "comparison failed the schema: {errors:#?}"
    );
}

struct FakeSearch {
    outcome: CandidateSearchOutcome,
    calls: Cell<usize>,
}

impl CandidateSearch for FakeSearch {
    fn search(&self, _query: &CohortQuery) -> Result<CandidateSearchOutcome, CandidateSearchError> {
        self.calls.set(self.calls.get() + 1);
        Ok(self.outcome.clone())
    }
}

fn candidate(
    repository: &str,
    stars: Option<u64>,
    facets: Vec<(&str, Vec<&str>)>,
) -> CandidateDescriptor {
    CandidateDescriptor::new(
        hosted("other-org", repository),
        revision("fedcba9876543210fedcba9876543210fedcba98"),
        false,
        facets
            .into_iter()
            .map(|(facet, tokens)| {
                (
                    facet.to_owned(),
                    tokens.into_iter().map(str::to_owned).collect(),
                )
            })
            .collect(),
        stars,
        evidence(&format!("evidence:github:candidate-{repository}")),
    )
    .unwrap()
}

fn golden_seed() -> SeedProject {
    let profile = ComparisonProfile::new(
        CohortMode::FunctionalCohort,
        vec![
            (
                "problem_overlap".to_owned(),
                vec![
                    "dependency_analysis".to_owned(),
                    "project_scoring".to_owned(),
                ],
            ),
            (
                "feature_overlap".to_owned(),
                vec!["cli".to_owned(), "json_output".to_owned()],
            ),
            ("technical_similarity".to_owned(), vec!["rust".to_owned()]),
        ],
        vec![evidence("evidence:repository:snapshot")],
    )
    .unwrap();
    SeedProject::new(
        hosted("example-org", "assay"),
        revision("0123456789abcdef0123456789abcdef01234567"),
        profile,
    )
}

fn golden_search() -> FakeSearch {
    let candidates = vec![
        candidate(
            "rival-a",
            Some(1_200),
            vec![
                (
                    "problem_overlap",
                    vec!["dependency_analysis", "project_scoring"],
                ),
                ("feature_overlap", vec!["cli", "web_dashboard"]),
                ("technical_similarity", vec!["rust"]),
            ],
        ),
        candidate(
            "rival-b",
            Some(30),
            vec![
                ("problem_overlap", vec!["project_scoring"]),
                ("technical_similarity", vec!["go"]),
            ],
        ),
        candidate(
            "rival-c",
            Some(5_000),
            vec![("feature_overlap", vec!["cli"])],
        ),
        candidate(
            "rival-d",
            Some(80),
            vec![("problem_overlap", vec!["dependency_analysis"])],
        ),
        candidate(
            "rival-e",
            Some(12),
            vec![("technical_similarity", vec!["rust"])],
        ),
        candidate(
            "rival-f",
            Some(7),
            vec![("feature_overlap", vec!["json_output"])],
        ),
        candidate(
            "unrelated",
            Some(4_000),
            vec![("structural_similarity", vec!["monorepo"])],
        ),
    ];
    FakeSearch {
        outcome: CandidateSearchOutcome::new(EvidenceStatus::Complete, candidates).unwrap(),
        calls: Cell::new(0),
    }
}

#[test]
fn comparison_reproduces_the_reviewed_golden_and_validates() {
    let search = golden_search();
    let comparison = discover_cohort(&golden_seed(), &search, &ComparisonPolicy::v1()).unwrap();
    assert_eq!(search.calls.get(), 1, "discovery issues exactly one search");
    let produced = comparison.to_machine_value();

    if std::env::var_os("ASSAY_EMIT_GOLDEN").is_some() {
        std::fs::write(
            repository_root().join("tests/golden/project-comparison-v1.json"),
            format!("{}\n", serde_json::to_string_pretty(&produced).unwrap()),
        )
        .unwrap();
    }

    let golden: Value = serde_json::from_str(include_str!(
        "../../../tests/golden/project-comparison-v1.json"
    ))
    .expect("reviewed comparison golden must parse");
    assert_eq!(
        produced, golden,
        "the discovery engine must reproduce the reviewed golden"
    );
    assert_schema_valid(&produced);
    // The golden exercises the full contract: five detailed candidates, one
    // compact additional candidate, and one candidate excluded as insufficient.
    assert_eq!(produced["detailed_candidates"].as_array().unwrap().len(), 5);
    assert_eq!(
        produced["additional_candidates"].as_array().unwrap().len(),
        1
    );
    assert_eq!(produced["status"], "partial");
}
