mod schema_contracts_helpers;
use serde_json::Value;

use schema_contracts_helpers::common::{assert_rejected, read_json, repository_root, validator};

#[test]
fn potential_has_a_distinct_forecast_contract_with_cited_context() {
    let root = repository_root();
    let validator = validator("project-evaluation");
    let evaluation = read_json(root.join("tests/golden/project-evaluation-v1.json"));
    let potential = &evaluation["scores"]["potential"];
    assert!(potential.get("forecast_horizon").is_some());
    assert!(potential.get("assumptions").is_some());
    assert!(potential.get("major_counter_signals").is_some());
    assert!(
        evaluation["scores"]["assay_score"]
            .get("forecast_horizon")
            .is_none(),
        "Assay Score must not absorb Potential forecast fields"
    );

    for required in ["forecast_horizon", "assumptions", "major_counter_signals"] {
        let mut missing = evaluation.clone();
        missing["scores"]["potential"]
            .as_object_mut()
            .expect("potential must be an object")
            .remove(required);
        assert_rejected(
            &validator,
            &missing,
            "project-evaluation",
            &format!("Potential missing {required}"),
        );
    }

    let mut invalid_horizon = evaluation.clone();
    invalid_horizon["scores"]["potential"]["forecast_horizon"] =
        Value::String("twelve_months".to_owned());
    assert_rejected(
        &validator,
        &invalid_horizon,
        "project-evaluation",
        "non-ISO-8601 Potential horizon",
    );

    schema_contracts_helpers::assertions::assert_golden_value_rejected(
        "project-evaluation",
        "/scores/potential/assumptions/0/evidence_ids",
        Value::Array(Vec::new()),
        "uncited Potential assumption",
    );

    let mut generic_score_with_forecast = evaluation;
    generic_score_with_forecast["scores"]["assay_score"]["forecast_horizon"] =
        Value::String("P1Y".to_owned());
    assert_rejected(
        &validator,
        &generic_score_with_forecast,
        "project-evaluation",
        "Assay Score with Potential-only fields",
    );
}

#[test]
fn potential_forecast_horizon_is_a_positive_iso_8601_duration() {
    const POINTER: &str = "/scores/potential/forecast_horizon";

    for zero_duration in [
        "P0D",
        "PT0S",
        "P0Y0M0DT0H0M0S",
        "PT0.0S",
        "P0Y0M0DT0H0M0.000S",
    ] {
        schema_contracts_helpers::assertions::assert_golden_value_rejected(
            "project-evaluation",
            POINTER,
            Value::String(zero_duration.to_owned()),
            "zero Potential forecast horizon",
        );
    }

    for positive_duration in ["P1D", "P1Y", "PT1H"] {
        schema_contracts_helpers::assertions::assert_golden_value_valid(
            "project-evaluation",
            POINTER,
            Value::String(positive_duration.to_owned()),
            "positive Potential forecast horizon",
        );
    }

    for invalid_duration in ["PT0.5S", "P1Dgarbage", "not-a-duration-1", "P1DT"] {
        schema_contracts_helpers::assertions::assert_golden_value_rejected(
            "project-evaluation",
            POINTER,
            Value::String(invalid_duration.to_owned()),
            "invalid Potential forecast horizon containing a non-zero digit",
        );
    }
}
