use std::collections::BTreeMap;

use assay_domain::{EvidenceId, EvidenceStatus, RubricApplicability};

use crate::scoring::contribution::ScoreContribution;
use crate::scoring::dimensions::{ASSAY_SCORE_DIMENSIONS, ESSENTIAL_DIMENSIONS, ScoreDimension};
use crate::scoring::scores::{AssayScore, DimensionScore, PotentialScore};
use crate::scoring::validation::sorted_unique;

use crate::scoring::compiler::ScoreCompilerInput;

impl ScoreCompilerInput {
    pub(crate) fn compile_dimension(
        &self,
        dimension: ScoreDimension,
        contributions: Vec<ScoreContribution>,
        version: &str,
    ) -> DimensionScore {
        let evidence_ids = sorted_unique(
            contributions
                .iter()
                .flat_map(|contribution| contribution.evidence_ids().iter().cloned())
                .collect(),
        );
        let scoreable = contributions
            .iter()
            .filter(|contribution| {
                contribution.applicability() != RubricApplicability::NotApplicable
                    && contribution.normalized_value().is_some()
            })
            .collect::<Vec<_>>();

        if scoreable.is_empty() {
            let status = if self.policy.is_essential(dimension) {
                EvidenceStatus::Insufficient
            } else {
                EvidenceStatus::Unavailable
            };
            return DimensionScore {
                dimension,
                status,
                value: None,
                confidence: 0.0,
                version: version.to_owned(),
                evidence_ids,
                contributions,
            };
        }

        let mut weight_sum = 0.0;
        let mut value_sum = 0.0;
        let mut confidence_sum = 0.0;
        for contribution in &scoreable {
            let weight = match contribution.applicability() {
                RubricApplicability::Applicable => 1.0,
                RubricApplicability::PartiallyApplicable => self.policy.partial_weight(),
                RubricApplicability::NotApplicable => continue,
            };
            weight_sum += weight;
            value_sum += weight * contribution.normalized_value().unwrap_or(0.0);
            confidence_sum += weight * contribution.confidence();
        }
        // The input contract gives every non-not_applicable contribution a
        // value, so a scoreable dimension has no per-contribution gap state.
        DimensionScore {
            dimension,
            status: EvidenceStatus::Complete,
            value: Some(value_sum / weight_sum * 100.0),
            confidence: confidence_sum / weight_sum,
            version: version.to_owned(),
            evidence_ids,
            contributions,
        }
    }

    pub(crate) fn compile_assay_score(
        &self,
        dimensions: &BTreeMap<ScoreDimension, DimensionScore>,
    ) -> AssayScore {
        let available = ASSAY_SCORE_DIMENSIONS
            .into_iter()
            .filter(|dimension| dimensions[dimension].value.is_some())
            .collect::<Vec<_>>();
        let essential_available = ESSENTIAL_DIMENSIONS
            .into_iter()
            .all(|dimension| dimensions[&dimension].value.is_some());
        let version = self.policy.score_version.to_owned();

        if !essential_available {
            let any_insufficient = ESSENTIAL_DIMENSIONS
                .into_iter()
                .any(|dimension| dimensions[&dimension].status == EvidenceStatus::Insufficient);
            let status = if any_insufficient {
                EvidenceStatus::Insufficient
            } else {
                EvidenceStatus::Unavailable
            };
            return AssayScore {
                status,
                value: None,
                confidence: 0.0,
                provisional: false,
                version,
                evidence_ids: Vec::new(),
            };
        }

        let mut weight_sum = 0.0;
        let mut value_sum = 0.0;
        let mut confidence_sum = 0.0;
        for dimension in &available {
            let score = &dimensions[dimension];
            let weight = self.policy.weight(*dimension);
            weight_sum += weight;
            value_sum += weight * score.value.unwrap_or(0.0);
            confidence_sum += weight * score.confidence;
        }
        let provisional = available.len() != ASSAY_SCORE_DIMENSIONS.len();
        let mut confidence = confidence_sum / weight_sum;
        if provisional {
            confidence *= self.policy.provisional_penalty();
        }
        let status = if provisional {
            EvidenceStatus::Partial
        } else {
            EvidenceStatus::Complete
        };
        let evidence_ids = sorted_unique(
            available
                .iter()
                .flat_map(|dimension| dimensions[dimension].evidence_ids.iter().cloned())
                .collect(),
        );
        AssayScore {
            status,
            value: Some(value_sum / weight_sum),
            confidence,
            provisional,
            version,
            evidence_ids,
        }
    }

    pub(crate) fn compile_potential(
        &self,
        contributions: Vec<ScoreContribution>,
    ) -> PotentialScore {
        let base = self.compile_dimension(
            ScoreDimension::Potential,
            contributions,
            self.policy.potential_version,
        );
        PotentialScore {
            status: base.status,
            value: base.value,
            confidence: base.confidence,
            version: self.policy.potential_version.to_owned(),
            evidence_ids: base.evidence_ids,
            forecast_horizon: self.policy.forecast_horizon.to_owned(),
            assumptions: self.potential_context.assumptions.clone(),
            major_counter_signals: self.potential_context.major_counter_signals.clone(),
            contributions: base.contributions,
        }
    }

    pub(crate) fn build_warnings(
        &self,
        assay_score: &AssayScore,
    ) -> Vec<(String, Vec<EvidenceId>)> {
        let mut warnings = Vec::new();
        if assay_score.status != EvidenceStatus::Complete {
            warnings.push(("score_release_gate_not_met".to_owned(), Vec::new()));
        }
        warnings
    }

    pub(crate) fn build_limitations(
        &self,
        assay_score: &AssayScore,
        dimensions: &BTreeMap<ScoreDimension, DimensionScore>,
    ) -> Vec<(String, Vec<EvidenceId>)> {
        let mut limitations = vec![(
            "repository_code_not_executed".to_owned(),
            self.classification.evidence_ids.clone(),
        )];
        if assay_score.provisional {
            limitations.push(("provisional_score_normalization".to_owned(), Vec::new()));
            let missing = sorted_unique(
                ASSAY_SCORE_DIMENSIONS
                    .into_iter()
                    .filter(|dimension| dimensions[dimension].value.is_none())
                    .flat_map(|dimension| dimensions[&dimension].evidence_ids.clone())
                    .collect(),
            );
            limitations.push(("missing_dimension_evidence".to_owned(), missing));
        }
        limitations.sort_by(|left, right| left.0.cmp(&right.0));
        limitations
    }

    pub(crate) fn collect_evidence(
        &self,
        dimensions: &BTreeMap<ScoreDimension, DimensionScore>,
        assay_score: &AssayScore,
        potential: &PotentialScore,
        limitations: &[(String, Vec<EvidenceId>)],
    ) -> Vec<EvidenceId> {
        let mut ids = self.classification.evidence_ids.clone();
        ids.extend(assay_score.evidence_ids.iter().cloned());
        for score in dimensions.values() {
            ids.extend(score.evidence_ids.iter().cloned());
        }
        ids.extend(potential.evidence_ids.iter().cloned());
        for statement in potential
            .assumptions
            .iter()
            .chain(&potential.major_counter_signals)
        {
            ids.extend(statement.evidence_ids.iter().cloned());
        }
        for (_, evidence) in limitations {
            ids.extend(evidence.iter().cloned());
        }
        sorted_unique(ids)
    }
}
