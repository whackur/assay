use std::collections::{BTreeMap, BTreeSet};

use assay_domain::{EvidenceId, EvidenceStatus, RepositorySource, RevisionId};
use serde_json::{Value, json};

use crate::comparison::candidate::{Candidate, Differentiator};
use crate::comparison::mapping::{
    SCHEMA_VERSION, diagnostics, evidence_values, jaccard_basis_points, repository_value,
    sorted_unique, status_code,
};
use crate::comparison::policy::{COMPARISON_VERSION, ComparisonPolicy};
use crate::comparison::types::{
    CandidateDescriptor, CandidateSearch, CohortMode, ComparisonError, ComparisonErrorKind,
    SearchDepth, SeedProject,
};

/// The compiled one-depth cohort comparison with its public machine mapping.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CohortComparison {
    pub(crate) mode: CohortMode,
    pub(crate) status: EvidenceStatus,
    pub(crate) seed_source: RepositorySource,
    pub(crate) seed_revision: RevisionId,
    pub(crate) facet_weights: BTreeMap<String, u32>,
    pub(crate) detailed: Vec<Candidate>,
    pub(crate) additional: Vec<Candidate>,
    pub(crate) evidence_ids: Vec<EvidenceId>,
    pub(crate) limitations: Vec<(String, Vec<EvidenceId>)>,
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
        .mode
        .canonical_facets()
        .iter()
        .map(|&facet| (facet.to_owned(), policy.facet_weight(facet)))
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
        // A candidate must earn at least one cited selection reason; a zero
        // overlap or facet-less result is an explicit insufficiency, not a
        // zero-similarity detailed entry.
        if candidate.overall_similarity_bp.is_none() || candidate.selection_reasons.is_empty() {
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

    // Every canonical facet is enumerated; a side without tokens for a facet
    // makes that facet explicitly unavailable, never a zero.
    let canonical = seed.profile.mode.canonical_facets();
    for &facet in canonical {
        let similarity = match (seed.profile.facets.get(facet), descriptor.facets.get(facet)) {
            (Some(seed_tokens), Some(candidate_tokens)) => {
                Some(jaccard_basis_points(seed_tokens, candidate_tokens))
            }
            _ => None,
        };
        if let Some(bp) = similarity {
            available += 1;
            let weight = u64::from(policy.facet_weight(facet));
            weight_sum += weight;
            value_sum += weight * u64::from(bp);
            if bp > 0 {
                selection_reasons.push(facet.to_owned());
            }
        }
        facet_similarities.insert(facet.to_owned(), similarity);
    }

    let overall_similarity_bp = (weight_sum > 0).then(|| (value_sum / weight_sum) as u32);
    let confidence_bp = ((available as u64 * 10_000) / canonical.len() as u64) as u32;
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
    // Non-canonical candidate facets are ignored entirely so their tokens
    // never leak into public differentiator output.
    let canonical = seed.profile.mode.canonical_facets();
    let seed_tokens = union_tokens(&seed.profile.facets, canonical);
    let candidate_tokens = union_tokens(&descriptor.facets, canonical);
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

fn union_tokens(
    facets: &BTreeMap<String, BTreeSet<String>>,
    canonical: &[&str],
) -> BTreeSet<String> {
    canonical
        .iter()
        .filter_map(|facet| facets.get(*facet))
        .flatten()
        .cloned()
        .collect()
}
