use serde_json::Value;

pub(super) fn validate_analysis_status(
    manifest: &Value,
    evidence: &[Value],
    data_sources: &[Value],
    artifacts: &[Value],
) -> Result<(), &'static str> {
    let status = manifest["status"].as_str().ok_or("manifest_status")?;
    let statuses = evidence
        .iter()
        .map(|value| &value["status"])
        .chain(data_sources.iter().map(|value| &value["status"]))
        .chain(std::iter::once(&manifest["scope"]["history_status"]));
    let all_complete = statuses.clone().all(|value| value == "complete");
    match status {
        "complete" => {
            if !all_complete || artifacts.iter().any(|value| value["status"] != "complete") {
                return Err("complete_status_contradiction");
            }
        }
        "partial" => {
            if all_complete {
                return Err("partial_status_without_limitation");
            }
        }
        _ => return Err("unsupported_analysis_status"),
    }
    Ok(())
}
