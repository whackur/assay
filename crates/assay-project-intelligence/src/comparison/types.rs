use std::collections::{BTreeMap, BTreeSet};

use assay_domain::{EvidenceId, EvidenceStatus, RepositorySource, RevisionId};

/// The cohort a project is compared within.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CohortMode {
    /// Projects that address a similar problem with a similar approach.
    FunctionalCohort,
    /// Curated lists compared as artifacts against other curated lists.
    CuratedList,
}

impl CohortMode {
    pub(crate) const fn code(self) -> &'static str {
        match self {
            Self::FunctionalCohort => "functional_cohort",
            Self::CuratedList => "curated_list",
        }
    }

    /// Returns the closed canonical facet set every comparison must enumerate.
    ///
    /// The specification's similarity dimensions are contract fields, not
    /// seed-dependent extras: a facet without data is explicit `unavailable`.
    /// Curated comparison carries the five criteria of specification 7.3,
    /// including maintenance evidence.
    pub const fn canonical_facets(self) -> &'static [&'static str] {
        match self {
            Self::FunctionalCohort => &[
                "problem_overlap",
                "feature_overlap",
                "technical_similarity",
                "structural_similarity",
            ],
            Self::CuratedList => &[
                "entry_overlap",
                "list_structure",
                "unique_coverage",
                "editorial_quality",
                "maintenance_evidence",
            ],
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
    pub(crate) const fn code(self) -> &'static str {
        match self {
            Self::OneDepth => "one_depth",
        }
    }
}

/// Stable, redacted comparison failure category.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ComparisonErrorKind {
    InvalidFacet,
    NonCanonicalFacet,
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
    pub(crate) const fn new(kind: ComparisonErrorKind) -> Self {
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

/// The seed project's cited facet token sets and comparison mode.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ComparisonProfile {
    pub(crate) mode: CohortMode,
    pub(crate) facets: BTreeMap<String, BTreeSet<String>>,
    pub(crate) evidence_ids: Vec<EvidenceId>,
}

impl ComparisonProfile {
    /// Validates the seed profile, requiring at least one non-empty facet.
    ///
    /// Facets are restricted to the mode's closed canonical set; custom facets
    /// are rejected so the published contract stays enumerable. Tokens are
    /// canonical snake_case machine codes so the comparison stays portable and
    /// free of raw source text. Empty token sets are dropped rather than
    /// compared as a zero.
    pub fn new(
        mode: CohortMode,
        facet_tokens: Vec<(String, Vec<String>)>,
        evidence_ids: Vec<EvidenceId>,
    ) -> Result<Self, ComparisonError> {
        let facets = crate::comparison::validation::validate_facets(facet_tokens)?;
        if facets
            .keys()
            .any(|facet| !mode.canonical_facets().contains(&facet.as_str()))
        {
            return Err(ComparisonError::new(ComparisonErrorKind::NonCanonicalFacet));
        }
        if facets.is_empty() || evidence_ids.is_empty() {
            return Err(ComparisonError::new(ComparisonErrorKind::EmptyProfile));
        }
        Ok(Self {
            mode,
            facets,
            evidence_ids: crate::comparison::mapping::sorted_unique(evidence_ids),
        })
    }
}

/// One candidate returned by the search port, with its own declared tokens.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CandidateDescriptor {
    pub(crate) source: RepositorySource,
    pub(crate) revision: RevisionId,
    pub(crate) curated: bool,
    pub(crate) facets: BTreeMap<String, BTreeSet<String>>,
    pub(crate) stars: Option<u64>,
    pub(crate) discovery_evidence_id: EvidenceId,
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
        let facets = crate::comparison::validation::validate_facets(facet_tokens)?;
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
    pub(crate) status: EvidenceStatus,
    pub(crate) candidates: Vec<CandidateDescriptor>,
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
    pub(crate) mode: CohortMode,
    pub(crate) facets: BTreeMap<String, BTreeSet<String>>,
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
    pub(crate) source: RepositorySource,
    pub(crate) revision: RevisionId,
    pub(crate) profile: ComparisonProfile,
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
