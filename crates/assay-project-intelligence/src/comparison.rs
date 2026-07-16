//! Deterministic one-depth functional-cohort discovery and comparison.
//!
//! Assay extracts a comparison profile from the analyzed project, asks a narrow
//! candidate-search port for public GitHub candidates exactly once, and compares
//! each candidate against the seed. Discovery stops at one search depth: a
//! discovered candidate carries no profile and cannot construct a
//! [`CohortQuery`], so it can never seed another discovery pass. The real GitHub
//! search wiring lives behind [`CandidateSearch`] and is deferred; this module
//! is exercised with deterministic fakes.
//!
//! Similarity is computed only from declared facet tokens with deterministic
//! integer arithmetic; identical input yields byte-identical output. Similarity
//! is never a quality signal and never implies misconduct. Popularity such as
//! star counts is retained as context and used only as an ordering tie-break;
//! it never raises a similarity value. An awesome list is compared as a curated
//! artifact against other curated lists, never by analyzing its linked projects.
//! Unavailable and insufficient comparisons remain explicit states, never zero.

use std::collections::{BTreeMap, BTreeSet};

use assay_domain::{EvidenceId, EvidenceStatus, RepositorySource, RevisionId};
use serde_json::{Value, json};

const SCHEMA_VERSION: &str = "1.0.0";
const COMPARISON_VERSION: &str = "project-comparison-1";

/// The cohort a project is compared within.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CohortMode {
    /// Projects that address a similar problem with a similar approach.
    FunctionalCohort,
    /// Curated lists compared as artifacts against other curated lists.
    CuratedList,
}

impl CohortMode {
    const fn code(self) -> &'static str {
        match self {
            Self::FunctionalCohort => "functional_cohort",
            Self::CuratedList => "curated_list",
        }
    }
}

/// The one-depth discovery guarantee recorded on every comparison.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SearchDepth {
    /// Exactly one search was issued and no candidate seeded another.
    OneDepth,
}

impl SearchDepth {
    const fn code(self) -> &'static str {
        match self {
            Self::OneDepth => "one_depth",
        }
    }
}

/// Stable, redacted comparison failure category.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ComparisonErrorKind {
    InvalidFacet,
    InvalidToken,
    EmptyProfile,
    CandidateNotHosted,
    UncitedCandidate,
    SearchFailed,
}

/// A redacted comparison failure that never echoes source or path material.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ComparisonError {
    kind: ComparisonErrorKind,
}

impl ComparisonError {
    const fn new(kind: ComparisonErrorKind) -> Self {
        Self { kind }
    }

    /// Returns the stable non-sensitive failure category.
    pub const fn kind(&self) -> ComparisonErrorKind {
        self.kind
    }
}

impl std::fmt::Display for ComparisonError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "cohort comparison failed ({:?})", self.kind)
    }
}

impl std::error::Error for ComparisonError {}

fn is_machine_code(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.is_empty() || bytes.len() > 64 || !bytes[0].is_ascii_lowercase() {
        return false;
    }
    let mut previous_separator = false;
    for &byte in bytes {
        if byte == b'_' {
            if previous_separator {
                return false;
            }
            previous_separator = true;
        } else if byte.is_ascii_lowercase() || byte.is_ascii_digit() {
            previous_separator = false;
        } else {
            return false;
        }
    }
    !previous_separator
}

fn validate_facets(
    facet_tokens: Vec<(String, Vec<String>)>,
) -> Result<BTreeMap<String, BTreeSet<String>>, ComparisonError> {
    let mut facets = BTreeMap::new();
    for (facet, tokens) in facet_tokens {
        if !is_machine_code(&facet) {
            return Err(ComparisonError::new(ComparisonErrorKind::InvalidFacet));
        }
        let mut token_set = BTreeSet::new();
        for token in tokens {
            if !is_machine_code(&token) {
                return Err(ComparisonError::new(ComparisonErrorKind::InvalidToken));
            }
            token_set.insert(token);
        }
        if !token_set.is_empty() {
            facets.insert(facet, token_set);
        }
    }
    Ok(facets)
}

/// The seed project's cited facet token sets and comparison mode.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ComparisonProfile {
    mode: CohortMode,
    facets: BTreeMap<String, BTreeSet<String>>,
    evidence_ids: Vec<EvidenceId>,
}

impl ComparisonProfile {
    /// Validates the seed profile, requiring at least one non-empty facet.
    ///
    /// Facet codes and tokens are canonical snake_case machine codes so the
    /// comparison stays portable and free of raw source text. Empty token sets
    /// are dropped rather than compared as a zero.
    pub fn new(
        mode: CohortMode,
        facet_tokens: Vec<(String, Vec<String>)>,
        evidence_ids: Vec<EvidenceId>,
    ) -> Result<Self, ComparisonError> {
        let facets = validate_facets(facet_tokens)?;
        if facets.is_empty() || evidence_ids.is_empty() {
            return Err(ComparisonError::new(ComparisonErrorKind::EmptyProfile));
        }
        Ok(Self {
            mode,
            facets,
            evidence_ids: sorted_unique(evidence_ids),
        })
    }
}

/// One candidate returned by the search port, with its own declared tokens.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CandidateDescriptor {
    source: RepositorySource,
    revision: RevisionId,
    curated: bool,
    facets: BTreeMap<String, BTreeSet<String>>,
    stars: Option<u64>,
    discovery_evidence_id: EvidenceId,
}

impl CandidateDescriptor {
    /// Validates one discovered candidate.
    ///
    /// A candidate must be a hosted GitHub repository and must cite the search
    /// evidence that surfaced it. `stars` is retained only as ordering context.
    pub fn new(
        source: RepositorySource,
        revision: RevisionId,
        curated: bool,
        facet_tokens: Vec<(String, Vec<String>)>,
        stars: Option<u64>,
        discovery_evidence_id: EvidenceId,
    ) -> Result<Self, ComparisonError> {
        if source.hosted_locator().is_none() {
            return Err(ComparisonError::new(
                ComparisonErrorKind::CandidateNotHosted,
            ));
        }
        let facets = validate_facets(facet_tokens)?;
        Ok(Self {
            source,
            revision,
            curated,
            facets,
            stars,
            discovery_evidence_id,
        })
    }
}

/// A usable-or-explicit search result from the candidate-search port.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CandidateSearchOutcome {
    status: EvidenceStatus,
    candidates: Vec<CandidateDescriptor>,
}

impl CandidateSearchOutcome {
    /// Builds a search outcome, binding candidate presence to a usable status.
    pub fn new(
        status: EvidenceStatus,
        candidates: Vec<CandidateDescriptor>,
    ) -> Result<Self, ComparisonError> {
        for candidate in &candidates {
            if candidate.discovery_evidence_id.as_str().is_empty() {
                return Err(ComparisonError::new(ComparisonErrorKind::UncitedCandidate));
            }
        }
        Ok(Self { status, candidates })
    }
}

/// A stable, redacted candidate-search transport failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CandidateSearchError;

/// The narrow, injectable candidate-search boundary.
///
/// The real implementation queries public GitHub search; deterministic fakes
/// implement it in tests. The port is invoked once per analyzed project and is
/// never handed a query derived from a discovered candidate.
pub trait CandidateSearch {
    /// Returns candidates for exactly one seed query.
    fn search(&self, query: &CohortQuery) -> Result<CandidateSearchOutcome, CandidateSearchError>;
}

/// A read-only query derived solely from the seed project.
///
/// Only [`SeedProject::query`] constructs this, and a discovered candidate never
/// yields one, so the search port cannot be re-entered from a candidate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CohortQuery {
    mode: CohortMode,
    facets: BTreeMap<String, BTreeSet<String>>,
}

impl CohortQuery {
    /// Returns the cohort mode the search must target.
    pub const fn mode(&self) -> CohortMode {
        self.mode
    }

    /// Returns the seed facet token sets that seed the search, in facet order.
    pub fn facet_tokens(&self) -> impl Iterator<Item = (&str, impl Iterator<Item = &str>)> {
        self.facets
            .iter()
            .map(|(facet, tokens)| (facet.as_str(), tokens.iter().map(String::as_str)))
    }
}

/// The analyzed project with its comparison profile.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SeedProject {
    source: RepositorySource,
    revision: RevisionId,
    profile: ComparisonProfile,
}

impl SeedProject {
    /// Binds the analyzed repository to its comparison profile.
    pub const fn new(
        source: RepositorySource,
        revision: RevisionId,
        profile: ComparisonProfile,
    ) -> Self {
        Self {
            source,
            revision,
            profile,
        }
    }

    /// Builds the one and only cohort query, derived from the seed.
    pub fn query(&self) -> CohortQuery {
        CohortQuery {
            mode: self.profile.mode,
            facets: self.profile.facets.clone(),
        }
    }
}

/// Versioned facet weights and cohort-size policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ComparisonPolicy {
    comparison_version: &'static str,
    detailed_limit: usize,
    default_facet_weight: u32,
}

impl ComparisonPolicy {
    /// Returns the initial versioned comparison policy.
    pub const fn v1() -> Self {
        Self {
            comparison_version: COMPARISON_VERSION,
            detailed_limit: 5,
            default_facet_weight: 10,
        }
    }

    fn facet_weight(&self, facet: &str) -> u32 {
        match facet {
            "problem_overlap" | "feature_overlap" | "entry_overlap" | "list_structure" => 30,
            "technical_similarity"
            | "structural_similarity"
            | "unique_coverage"
            | "editorial_quality" => 20,
            _ => self.default_facet_weight,
        }
    }
}

/// One differentiating token attributed to the seed or a candidate.
#[derive(Clone, Debug, Eq, PartialEq)]
struct Differentiator {
    token: String,
    evidence_ids: Vec<EvidenceId>,
}

impl Differentiator {
    fn to_value(&self) -> Value {
        json!({ "token": self.token, "evidence_ids": evidence_values(&self.evidence_ids) })
    }
}

/// One compared candidate with its facet breakdown and differentiators.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Candidate {
    source: RepositorySource,
    revision: RevisionId,
    stars: Option<u64>,
    discovery_evidence_id: EvidenceId,
    overall_similarity_bp: Option<u32>,
    confidence_bp: u32,
    selection_reasons: Vec<String>,
    facet_similarities: BTreeMap<String, Option<u32>>,
    seed_only: Vec<Differentiator>,
    candidate_only: Vec<Differentiator>,
    evidence_ids: Vec<EvidenceId>,
}

impl Candidate {
    /// Returns the overall similarity in the closed unit interval, when compared.
    pub fn overall_similarity(&self) -> Option<f64> {
        self.overall_similarity_bp.map(basis_points_to_unit)
    }

    /// Returns the canonical `provider/namespace/repository` identifier.
    pub fn identifier(&self) -> String {
        source_identifier(&self.source)
    }

    fn detailed_value(&self) -> Value {
        json!({
            "candidate": { "source": repository_value(&self.source), "revision": self.revision.as_str() },
            "selection_reasons": self.selection_reasons,
            "confidence": basis_points_to_unit(self.confidence_bp),
            "overall_similarity": similarity_value(self.overall_similarity_bp),
            "facets": self.facet_similarities.iter().map(|(facet, bp)| json!({
                "facet": facet,
                "status": similarity_status(*bp),
                "value": bp.map(basis_points_to_unit),
            })).collect::<Vec<_>>(),
            "popularity": { "stars": self.stars },
            "differentiators": {
                "seed_only": self.seed_only.iter().map(Differentiator::to_value).collect::<Vec<_>>(),
                "candidate_only": self.candidate_only.iter().map(Differentiator::to_value).collect::<Vec<_>>(),
            },
            "evidence_ids": evidence_values(&self.evidence_ids),
        })
    }

    fn compact_value(&self) -> Value {
        json!({
            "candidate": { "source": repository_value(&self.source), "revision": self.revision.as_str() },
            "confidence": basis_points_to_unit(self.confidence_bp),
            "overall_similarity": similarity_value(self.overall_similarity_bp),
            "evidence_ids": evidence_values(std::slice::from_ref(&self.discovery_evidence_id)),
        })
    }
}

/// The compiled one-depth cohort comparison with its public machine mapping.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CohortComparison {
    mode: CohortMode,
    status: EvidenceStatus,
    seed_source: RepositorySource,
    seed_revision: RevisionId,
    facet_weights: BTreeMap<String, u32>,
    detailed: Vec<Candidate>,
    additional: Vec<Candidate>,
    evidence_ids: Vec<EvidenceId>,
    limitations: Vec<(String, Vec<EvidenceId>)>,
}

impl CohortComparison {
    /// Returns the discovery depth guarantee; always one depth.
    pub const fn search_depth(&self) -> SearchDepth {
        SearchDepth::OneDepth
    }

    /// Returns comparison availability; unavailable is never a zero similarity.
    pub const fn status(&self) -> EvidenceStatus {
        self.status
    }

    /// Returns the detailed candidates, at most the policy detail limit.
    pub fn detailed_candidates(&self) -> &[Candidate] {
        &self.detailed
    }

    /// Returns the compact additional candidates beyond the detail limit.
    pub fn additional_candidates(&self) -> &[Candidate] {
        &self.additional
    }

    /// Maps the comparison onto `schemas/project-comparison/v1.json`.
    pub fn to_machine_value(&self) -> Value {
        json!({
            "schema_version": SCHEMA_VERSION,
            "comparison_version": COMPARISON_VERSION,
            "mode": self.mode.code(),
            "status": status_code(self.status),
            "search_depth": self.search_depth().code(),
            "seed": {
                "source": repository_value(&self.seed_source),
                "revision": self.seed_revision.as_str(),
            },
            "facet_weights": self.facet_weights.iter().map(|(facet, weight)| json!({
                "facet": facet,
                "weight": weight,
            })).collect::<Vec<_>>(),
            "detailed_candidates": self.detailed.iter().map(Candidate::detailed_value).collect::<Vec<_>>(),
            "additional_candidates": self.additional.iter().map(Candidate::compact_value).collect::<Vec<_>>(),
            "evidence_ids": evidence_values(&self.evidence_ids),
            "warnings": [],
            "limitations": diagnostics(&self.limitations),
        })
    }
}

/// Discovers and compares a one-depth functional cohort for one seed project.
///
/// The search port is invoked exactly once with the seed query. Discovered
/// candidates are compared but never re-searched, so discovery terminates at one
/// depth by construction.
pub fn discover_cohort(
    seed: &SeedProject,
    search: &impl CandidateSearch,
    policy: &ComparisonPolicy,
) -> Result<CohortComparison, ComparisonError> {
    let outcome = search
        .search(&seed.query())
        .map_err(|_| ComparisonError::new(ComparisonErrorKind::SearchFailed))?;

    let facet_weights = seed
        .profile
        .facets
        .keys()
        .map(|facet| (facet.clone(), policy.facet_weight(facet)))
        .collect::<BTreeMap<_, _>>();

    let mut limitations: Vec<(String, Vec<EvidenceId>)> = Vec::new();
    let usable = matches!(
        outcome.status,
        EvidenceStatus::Complete | EvidenceStatus::Partial
    );
    if !usable {
        return Ok(CohortComparison {
            mode: seed.profile.mode,
            status: outcome.status,
            seed_source: seed.source.clone(),
            seed_revision: seed.revision.clone(),
            facet_weights,
            detailed: Vec::new(),
            additional: Vec::new(),
            evidence_ids: seed.profile.evidence_ids.clone(),
            limitations,
        });
    }

    let mut ranked = Vec::new();
    for descriptor in &outcome.candidates {
        if seed.profile.mode == CohortMode::CuratedList && !descriptor.curated {
            limitations.push((
                "non_curated_candidate_excluded".to_owned(),
                vec![descriptor.discovery_evidence_id.clone()],
            ));
            continue;
        }
        let candidate = compare_candidate(seed, descriptor, policy);
        if candidate.overall_similarity_bp.is_none() {
            limitations.push((
                "candidate_similarity_insufficient".to_owned(),
                vec![descriptor.discovery_evidence_id.clone()],
            ));
            continue;
        }
        ranked.push(candidate);
    }

    ranked.sort_by(|left, right| {
        right
            .overall_similarity_bp
            .cmp(&left.overall_similarity_bp)
            .then(right.stars.unwrap_or(0).cmp(&left.stars.unwrap_or(0)))
            .then(left.identifier().cmp(&right.identifier()))
    });

    let dropped = !limitations.is_empty();
    let additional = ranked.split_off(ranked.len().min(policy.detailed_limit));
    let detailed = ranked;

    let status = if detailed.is_empty() {
        EvidenceStatus::Insufficient
    } else if outcome.status == EvidenceStatus::Complete && !dropped {
        EvidenceStatus::Complete
    } else {
        EvidenceStatus::Partial
    };

    let mut evidence_ids = seed.profile.evidence_ids.clone();
    for candidate in detailed.iter().chain(&additional) {
        evidence_ids.extend(candidate.evidence_ids.iter().cloned());
    }
    for (_, ids) in &limitations {
        evidence_ids.extend(ids.iter().cloned());
    }
    limitations.sort();

    Ok(CohortComparison {
        mode: seed.profile.mode,
        status,
        seed_source: seed.source.clone(),
        seed_revision: seed.revision.clone(),
        facet_weights,
        detailed,
        additional,
        evidence_ids: sorted_unique(evidence_ids),
        limitations,
    })
}

fn compare_candidate(
    seed: &SeedProject,
    descriptor: &CandidateDescriptor,
    policy: &ComparisonPolicy,
) -> Candidate {
    let mut facet_similarities = BTreeMap::new();
    let mut weight_sum = 0u64;
    let mut value_sum = 0u64;
    let mut available = 0usize;
    let mut selection_reasons = Vec::new();

    for (facet, seed_tokens) in &seed.profile.facets {
        let similarity = descriptor
            .facets
            .get(facet)
            .map(|candidate_tokens| jaccard_basis_points(seed_tokens, candidate_tokens));
        if let Some(bp) = similarity {
            available += 1;
            let weight = u64::from(policy.facet_weight(facet));
            weight_sum += weight;
            value_sum += weight * u64::from(bp);
            if bp > 0 {
                selection_reasons.push(facet.clone());
            }
        }
        facet_similarities.insert(facet.clone(), similarity);
    }

    let overall_similarity_bp = (weight_sum > 0).then(|| (value_sum / weight_sum) as u32);
    let confidence_bp = ((available as u64 * 10_000) / seed.profile.facets.len() as u64) as u32;
    selection_reasons.sort();

    let (seed_only, candidate_only) = differentiators(seed, descriptor);
    let mut evidence_ids = vec![descriptor.discovery_evidence_id.clone()];
    if !seed_only.is_empty() {
        evidence_ids.extend(seed.profile.evidence_ids.iter().cloned());
    }

    Candidate {
        source: descriptor.source.clone(),
        revision: descriptor.revision.clone(),
        stars: descriptor.stars,
        discovery_evidence_id: descriptor.discovery_evidence_id.clone(),
        overall_similarity_bp,
        confidence_bp,
        selection_reasons,
        facet_similarities,
        seed_only,
        candidate_only,
        evidence_ids: sorted_unique(evidence_ids),
    }
}

fn differentiators(
    seed: &SeedProject,
    descriptor: &CandidateDescriptor,
) -> (Vec<Differentiator>, Vec<Differentiator>) {
    let seed_tokens = union_tokens(&seed.profile.facets);
    let candidate_tokens = union_tokens(&descriptor.facets);
    let seed_only = seed_tokens
        .difference(&candidate_tokens)
        .map(|token| Differentiator {
            token: token.clone(),
            evidence_ids: seed.profile.evidence_ids.clone(),
        })
        .collect();
    let candidate_only = candidate_tokens
        .difference(&seed_tokens)
        .map(|token| Differentiator {
            token: token.clone(),
            evidence_ids: vec![descriptor.discovery_evidence_id.clone()],
        })
        .collect();
    (seed_only, candidate_only)
}

fn union_tokens(facets: &BTreeMap<String, BTreeSet<String>>) -> BTreeSet<String> {
    facets.values().flatten().cloned().collect()
}

fn jaccard_basis_points(left: &BTreeSet<String>, right: &BTreeSet<String>) -> u32 {
    let intersection = left.intersection(right).count() as u64;
    let union = left.union(right).count() as u64;
    if union == 0 {
        return 0;
    }
    ((intersection * 10_000) / union) as u32
}

fn basis_points_to_unit(bp: u32) -> f64 {
    f64::from(bp) / 10_000.0
}

fn similarity_value(bp: Option<u32>) -> Value {
    json!({ "status": similarity_status(bp), "value": bp.map(basis_points_to_unit) })
}

fn similarity_status(bp: Option<u32>) -> &'static str {
    if bp.is_some() {
        "complete"
    } else {
        "unavailable"
    }
}

fn diagnostics(entries: &[(String, Vec<EvidenceId>)]) -> Value {
    Value::Array(
        entries
            .iter()
            .map(|(code, evidence_ids)| {
                json!({ "code": code, "evidence_ids": evidence_values(evidence_ids) })
            })
            .collect(),
    )
}

fn evidence_values(evidence_ids: &[EvidenceId]) -> Vec<&str> {
    evidence_ids.iter().map(EvidenceId::as_str).collect()
}

fn source_identifier(source: &RepositorySource) -> String {
    if let Some((provider, namespace, repository)) = source.hosted_locator() {
        format!("{provider}/{namespace}/{repository}")
    } else if let Some(id) = source.local_repository_id() {
        format!("local/{}", id.as_str())
    } else {
        unreachable!("repository source variants are closed")
    }
}

fn repository_value(source: &RepositorySource) -> Value {
    if let Some(id) = source.local_repository_id() {
        json!({ "kind": "local", "repository_id": id.as_str() })
    } else if let Some((provider, namespace, repository)) = source.hosted_locator() {
        json!({ "kind": "hosted", "provider": provider, "namespace": namespace, "repository": repository })
    } else {
        unreachable!("repository source variants are closed")
    }
}

fn sorted_unique(mut ids: Vec<EvidenceId>) -> Vec<EvidenceId> {
    ids.sort();
    ids.dedup();
    ids
}

const fn status_code(status: EvidenceStatus) -> &'static str {
    match status {
        EvidenceStatus::Complete => "complete",
        EvidenceStatus::Partial => "partial",
        EvidenceStatus::Unavailable => "unavailable",
        EvidenceStatus::Unsupported => "unsupported",
        EvidenceStatus::Insufficient => "insufficient",
        EvidenceStatus::Pending => "pending",
    }
}

#[cfg(test)]
mod tests {
    use std::{cell::Cell, str::FromStr};

    use super::*;

    fn evidence(value: &str) -> EvidenceId {
        EvidenceId::from_str(value).unwrap()
    }

    fn revision() -> RevisionId {
        RevisionId::from_str("0123456789abcdef0123456789abcdef01234567").unwrap()
    }

    fn hosted(namespace: &str, repository: &str) -> RepositorySource {
        RepositorySource::hosted("github", namespace, repository).unwrap()
    }

    fn seed_profile() -> ComparisonProfile {
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

    fn seed() -> SeedProject {
        SeedProject::new(hosted("example-org", "seed"), revision(), seed_profile())
    }

    struct FakeSearch {
        outcome: CandidateSearchOutcome,
        calls: Cell<usize>,
    }

    impl CandidateSearch for FakeSearch {
        fn search(
            &self,
            _query: &CohortQuery,
        ) -> Result<CandidateSearchOutcome, CandidateSearchError> {
            self.calls.set(self.calls.get() + 1);
            Ok(self.outcome.clone())
        }
    }

    fn candidate(
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
}
