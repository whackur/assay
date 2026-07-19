#![cfg(unix)]
//! Schema validator and citation audit helpers for the vertical slice tests.

use std::{collections::BTreeMap, fs};

use jsonschema::{Draft, Registry, Resource};
use serde_json::Value;

use super::common::repository_root;

pub(crate) fn project_analysis_validator() -> jsonschema::Validator {
    let root = repository_root();
    let read = |name: &str| {
        serde_json::from_slice::<Value>(
            &fs::read(root.join("schemas").join(name).join("v1.json"))
                .unwrap_or_else(|error| panic!("read {name} schema: {error}")),
        )
        .unwrap_or_else(|error| panic!("parse {name} schema: {error}"))
    };
    let schema = read("project-analysis");
    let resources = [
        "analysis-manifest",
        "project-evidence",
        "project-evaluation",
    ]
    .into_iter()
    .map(|name| {
        let component = read(name);
        let id = component["$id"]
            .as_str()
            .expect("component schema ID")
            .to_owned();
        let resource = Resource::from_contents(component);
        (id, resource)
    });
    let registry = Registry::new()
        .extend(resources)
        .expect("component schema URIs")
        .prepare()
        .expect("component schema registry");
    jsonschema::options()
        .with_draft(Draft::Draft202012)
        .with_registry(&registry)
        .should_validate_formats(true)
        .build(&schema)
        .expect("project-analysis validator")
}

pub(crate) fn audit_bundle_citations(bundle: &Value) -> Result<(), String> {
    let evidence = bundle["evidence"]
        .as_array()
        .ok_or_else(|| "missing evidence".to_owned())?;
    let by_id = evidence
        .iter()
        .map(|record| {
            record["id"]
                .as_str()
                .map(|id| (id, record))
                .ok_or_else(|| "missing evidence ID".to_owned())
        })
        .collect::<Result<BTreeMap<_, _>, _>>()?;
    if by_id.len() != evidence.len() {
        return Err("duplicate evidence ID".into());
    }
    let require_reference = |reference: &Value, field: &str| -> Result<&Value, String> {
        let id = reference
            .as_str()
            .ok_or_else(|| format!("non-string citation in {field}"))?;
        by_id
            .get(id)
            .copied()
            .ok_or_else(|| format!("dangling citation in {field}: {id}"))
    };
    let require_references = |references: &Value, field: &str| -> Result<Vec<&Value>, String> {
        references
            .as_array()
            .ok_or_else(|| format!("missing citation list: {field}"))?
            .iter()
            .map(|reference| require_reference(reference, field))
            .collect()
    };
    for record in evidence {
        if let Some(related) = record.get("related_evidence_ids") {
            require_references(related, "evidence.related_evidence_ids")?;
        }
        let Some(payload) = record.get("payload") else {
            continue;
        };
        for field in ["related_evidence_ids", "implementation_evidence_ids"] {
            if let Some(references) = payload.get(field) {
                require_references(references, field)?;
            }
        }
        if let Some(source) = payload.get("source_evidence_id") {
            require_reference(source, "payload.source_evidence_id")?;
        }
        match payload["kind"].as_str() {
            Some("file_classification") => {
                let source = payload
                    .get("source_evidence_id")
                    .ok_or_else(|| "classification source citation missing".to_owned())?;
                let source_record = require_reference(source, "payload.source_evidence_id")?;
                let related = record["related_evidence_ids"]
                    .as_array()
                    .ok_or_else(|| "classification top-level relation missing".to_owned())?;
                if related.as_slice() != std::slice::from_ref(source) {
                    return Err("classification source relation mismatch".into());
                }
                let source_kind = source_record
                    .get("payload")
                    .and_then(|value| value["kind"].as_str())
                    .or_else(|| source_record["requested_kind"].as_str());
                if source_kind != Some("tracked_file") {
                    return Err("classification source is not tracked-file evidence".into());
                }
            }
            Some("repository_feature") => {
                let related = payload["related_evidence_ids"]
                    .as_array()
                    .ok_or_else(|| "feature citation list missing".to_owned())?;
                match payload["state"].as_str() {
                    Some("present" | "unavailable") if related.is_empty() => {
                        return Err("reviewable feature state has no citation".into());
                    }
                    Some("absent") if !related.is_empty() => {
                        return Err("absent feature has citations".into());
                    }
                    Some("present" | "unavailable" | "absent") => {}
                    _ => return Err("unknown repository feature state".into()),
                }
            }
            Some("claim_correspondence") => {
                require_references(
                    payload
                        .get("implementation_evidence_ids")
                        .ok_or_else(|| "claim citations missing".to_owned())?,
                    "payload.implementation_evidence_ids",
                )?;
            }
            _ => {}
        }
    }
    let manifest = bundle
        .get("manifest")
        .ok_or_else(|| "missing manifest".to_owned())?;
    for source in manifest["data_sources"]
        .as_array()
        .ok_or_else(|| "missing data sources".to_owned())?
    {
        require_reference(&source["id"], "manifest.data_sources.id")?;
    }
    for (kind, diagnostics) in [
        ("warning", &manifest["warnings"]),
        ("limitation", &manifest["limitations"]),
    ] {
        for diagnostic in diagnostics
            .as_array()
            .ok_or_else(|| format!("missing {kind} array"))?
        {
            let references = diagnostic
                .get("affected_evidence_ids")
                .ok_or_else(|| format!("{kind} citations missing"))?;
            let resolved = require_references(references, "diagnostic.affected_evidence_ids")?;
            if resolved.is_empty() {
                return Err(format!("{kind} citations empty"));
            }
        }
    }
    Ok(())
}
