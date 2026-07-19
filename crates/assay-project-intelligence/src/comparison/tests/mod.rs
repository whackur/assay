mod cohort;
mod profile;

use std::{cell::Cell, str::FromStr};

use assay_domain::{EvidenceId, RepositorySource, RevisionId};

use crate::comparison::types::{
    CandidateDescriptor, CandidateSearch, CandidateSearchError, CandidateSearchOutcome, CohortMode,
    CohortQuery, ComparisonProfile, SeedProject,
};

pub(crate) fn evidence(value: &str) -> EvidenceId {
    EvidenceId::from_str(value).unwrap()
}

pub(crate) fn revision() -> RevisionId {
    RevisionId::from_str("0123456789abcdef0123456789abcdef01234567").unwrap()
}

pub(crate) fn hosted(namespace: &str, repository: &str) -> RepositorySource {
    RepositorySource::hosted("github", namespace, repository).unwrap()
}

pub(crate) fn seed_profile() -> ComparisonProfile {
    ComparisonProfile::new(
        CohortMode::FunctionalCohort,
        vec![
            (
                "problem_overlap".to_owned(),
                vec!["dependency_analysis".to_owned(), "scoring".to_owned()],
            ),
            (
                "feature_overlap".to_owned(),
                vec!["cli".to_owned(), "json_output".to_owned()],
            ),
        ],
        vec![evidence("evidence:repository:snapshot")],
    )
    .unwrap()
}

pub(crate) fn seed() -> SeedProject {
    SeedProject::new(hosted("example-org", "seed"), revision(), seed_profile())
}

pub(crate) struct FakeSearch {
    pub(crate) outcome: CandidateSearchOutcome,
    pub(crate) calls: Cell<usize>,
}

impl CandidateSearch for FakeSearch {
    fn search(&self, _query: &CohortQuery) -> Result<CandidateSearchOutcome, CandidateSearchError> {
        self.calls.set(self.calls.get() + 1);
        Ok(self.outcome.clone())
    }
}

pub(crate) fn candidate(
    repository: &str,
    stars: Option<u64>,
    facet_tokens: Vec<(&str, Vec<&str>)>,
) -> CandidateDescriptor {
    CandidateDescriptor::new(
        hosted("other-org", repository),
        revision(),
        false,
        facet_tokens
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
