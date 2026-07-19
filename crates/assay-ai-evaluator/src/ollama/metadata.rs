use std::str::FromStr;

use assay_domain::EvidenceId;
use serde_json::Value;

use crate::{
    EvaluationError, EvidenceBundle, EvidenceDescriptor, EvidenceKind, EvidenceScope,
    ExternalTransmission,
};

/// Builds only bounded deterministic facts; repository descriptions and raw provider text are excluded.
pub fn build_hosted_metadata_bundle(facts: &Value) -> Result<EvidenceBundle, EvaluationError> {
    let mut items = Vec::new();
    push_u64(&mut items, facts, "stargazers_count", "stargazers");
    push_u64(&mut items, facts, "forks_count", "forks");
    push_u64(&mut items, facts, "open_issues_count", "open-issues");
    push_bool(&mut items, facts, "archived", "archived");
    push_bool(&mut items, facts, "fork", "fork");
    if let Some(value) = facts.get("head_sha").and_then(Value::as_str)
        && value.len() == 40
        && value.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        push_descriptor(
            &mut items,
            "evidence:github:head-sha",
            &format!(
                "GitHub resolved the default branch to commit {}.",
                value.to_ascii_lowercase()
            ),
        );
    }
    if let Some(value) = facts.get("license_spdx").and_then(Value::as_str)
        && !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'+'))
    {
        push_descriptor(
            &mut items,
            "evidence:github:license-spdx",
            &format!("GitHub reports the SPDX license identifier {value}."),
        );
    }
    EvidenceBundle::new(
        EvidenceScope::PublicOnly,
        ExternalTransmission::PublicOnly,
        items,
    )
}

fn push_u64(items: &mut Vec<EvidenceDescriptor>, facts: &Value, key: &str, id: &str) {
    if let Some(value) = facts.get(key).and_then(Value::as_u64) {
        push_descriptor(
            items,
            &format!("evidence:github:{id}"),
            &format!("GitHub reports {value} for {key}."),
        );
    }
}

fn push_bool(items: &mut Vec<EvidenceDescriptor>, facts: &Value, key: &str, id: &str) {
    if let Some(value) = facts.get(key).and_then(Value::as_bool) {
        push_descriptor(
            items,
            &format!("evidence:github:{id}"),
            &format!("GitHub reports {key} as {value}."),
        );
    }
}

fn push_descriptor(items: &mut Vec<EvidenceDescriptor>, id: &str, statement: &str) {
    let id = EvidenceId::from_str(id).expect("hard-coded evidence identifier is valid");
    if let Ok(descriptor) = EvidenceDescriptor::new(id, EvidenceKind::RepositoryFact, statement) {
        items.push(descriptor);
    }
}
