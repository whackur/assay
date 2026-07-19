use assay_domain::{EvidenceId, RubricApplicability};

pub(crate) const MAX_STATEMENT_BYTES: usize = 1_000;

pub(crate) fn is_version_identifier(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.is_empty() || bytes.len() > 100 {
        return false;
    }
    let boundary = |byte: u8| byte.is_ascii_lowercase() || byte.is_ascii_digit();
    boundary(bytes[0])
        && boundary(bytes[bytes.len() - 1])
        && bytes
            .iter()
            .all(|byte| boundary(*byte) || matches!(byte, b'.' | b'_' | b'-'))
}

pub(crate) fn is_machine_code(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.is_empty() || bytes.len() > 100 || !bytes[0].is_ascii_lowercase() {
        return false;
    }
    let mut previous_separator = true;
    for &byte in bytes {
        if matches!(byte, b'.' | b'_' | b'-') {
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

pub(crate) fn is_statement(value: &str) -> bool {
    !value.is_empty() && value.len() <= MAX_STATEMENT_BYTES && !value.chars().any(char::is_control)
}

pub(crate) fn sorted_unique(mut ids: Vec<EvidenceId>) -> Vec<EvidenceId> {
    ids.sort();
    ids.dedup();
    ids
}

pub(crate) fn validate_normalized(
    applicability: RubricApplicability,
    value: Option<f64>,
    confidence: f64,
    evidence_ids: &[EvidenceId],
) -> Result<(), ()> {
    if !confidence.is_finite() || !(0.0..=1.0).contains(&confidence) {
        return Err(());
    }
    match (applicability, value) {
        (RubricApplicability::NotApplicable, Some(_)) => return Err(()),
        (RubricApplicability::NotApplicable, None) => {}
        (_, None) => return Err(()),
        (_, Some(value)) if !value.is_finite() || !(0.0..=1.0).contains(&value) => return Err(()),
        (_, Some(_)) => {}
    }
    if applicability != RubricApplicability::NotApplicable && evidence_ids.is_empty() {
        return Err(());
    }
    Ok(())
}
