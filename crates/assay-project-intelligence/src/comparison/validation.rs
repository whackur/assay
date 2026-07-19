use std::collections::{BTreeMap, BTreeSet};

use crate::comparison::types::{ComparisonError, ComparisonErrorKind};

pub(crate) fn is_machine_code(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.is_empty() || bytes.len() > 64 || !bytes[0].is_ascii_lowercase() {
        return false;
    }
    let mut previous_separator = false;
    for &byte in bytes {
        if byte == b'_' {
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

pub(crate) fn validate_facets(
    facet_tokens: Vec<(String, Vec<String>)>,
) -> Result<BTreeMap<String, BTreeSet<String>>, ComparisonError> {
    let mut facets = BTreeMap::new();
    for (facet, tokens) in facet_tokens {
        if !is_machine_code(&facet) {
            return Err(ComparisonError::new(ComparisonErrorKind::InvalidFacet));
        }
        let mut token_set = BTreeSet::new();
        for token in tokens {
            if !is_machine_code(&token) {
                return Err(ComparisonError::new(ComparisonErrorKind::InvalidToken));
            }
            token_set.insert(token);
        }
        if !token_set.is_empty() {
            facets.insert(facet, token_set);
        }
    }
    Ok(facets)
}
