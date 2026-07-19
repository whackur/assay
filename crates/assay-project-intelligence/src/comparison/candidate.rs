use std::collections::BTreeMap;

use assay_domain::{EvidenceId, RepositorySource, RevisionId};
use serde_json::{Value, json};

use crate::comparison::mapping::{
    basis_points_to_unit, evidence_values, repository_value, similarity_status, similarity_value,
    source_identifier,
};

/// One differentiating token attributed to the seed or a candidate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Differentiator {
    pub(crate) token: String,
    pub(crate) evidence_ids: Vec<EvidenceId>,
}

impl Differentiator {
    pub(crate) fn to_value(&self) -> Value {
        json!({ "token": self.token, "evidence_ids": evidence_values(&self.evidence_ids) })
    }
}

/// One compared candidate with its facet breakdown and differentiators.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Candidate {
    pub(crate) source: RepositorySource,
    pub(crate) revision: RevisionId,
    pub(crate) stars: Option<u64>,
    pub(crate) discovery_evidence_id: EvidenceId,
    pub(crate) overall_similarity_bp: Option<u32>,
    pub(crate) confidence_bp: u32,
    pub(crate) selection_reasons: Vec<String>,
    pub(crate) facet_similarities: BTreeMap<String, Option<u32>>,
    pub(crate) seed_only: Vec<Differentiator>,
    pub(crate) candidate_only: Vec<Differentiator>,
    pub(crate) evidence_ids: Vec<EvidenceId>,
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

    pub(crate) fn detailed_value(&self) -> Value {
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

    pub(crate) fn compact_value(&self) -> Value {
        json!({
            "candidate": { "source": repository_value(&self.source), "revision": self.revision.as_str() },
            "confidence": basis_points_to_unit(self.confidence_bp),
            "overall_similarity": similarity_value(self.overall_similarity_bp),
            "evidence_ids": evidence_values(std::slice::from_ref(&self.discovery_evidence_id)),
        })
    }
}
