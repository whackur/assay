use std::collections::BTreeMap;

use serde_json::Value;

pub(super) fn validate_references(
    by_id: &BTreeMap<&str, &Value>,
    manifest: &Value,
) -> Result<(), &'static str> {
    for source in manifest["data_sources"]
        .as_array()
        .ok_or("missing_data_sources")?
    {
        require_reference(by_id, &source["id"])?;
    }
    for diagnostics in [&manifest["warnings"], &manifest["limitations"]] {
        for diagnostic in diagnostics.as_array().ok_or("invalid_diagnostics")? {
            require_references(by_id, &diagnostic["affected_evidence_ids"])?;
        }
    }
    for fact in by_id.values() {
        if let Some(related) = fact.get("related_evidence_ids") {
            require_references(by_id, related)?;
        }
        if let Some(payload) = fact.get("payload") {
            for field in [
                "related_evidence_ids",
                "implementation_evidence_ids",
                "source_evidence_id",
            ] {
                if let Some(reference) = payload.get(field) {
                    if reference.is_array() {
                        require_references(by_id, reference)?;
                    } else {
                        require_reference(by_id, reference)?;
                    }
                }
            }
        }
    }
    Ok(())
}

fn require_references(
    by_id: &BTreeMap<&str, &Value>,
    references: &Value,
) -> Result<(), &'static str> {
    for reference in references.as_array().ok_or("invalid_reference_list")? {
        require_reference(by_id, reference)?;
    }
    Ok(())
}

fn require_reference(
    by_id: &BTreeMap<&str, &Value>,
    reference: &Value,
) -> Result<(), &'static str> {
    let id = reference.as_str().ok_or("invalid_reference")?;
    if by_id.contains_key(id) {
        Ok(())
    } else {
        Err("unknown_evidence_reference")
    }
}
