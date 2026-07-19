use crate::{EvaluationError, EvaluationErrorKind};

const MAX_EVIDENCE_STATEMENT_BYTES: usize = 1_000;

#[derive(Clone, Copy)]
pub(crate) enum TextPolicy {
    Evidence,
    ProviderRationale,
}

pub(crate) fn validate_untrusted_text(
    value: &str,
    _policy: TextPolicy,
) -> Result<(), EvaluationError> {
    if value.is_empty()
        || value.len() > MAX_EVIDENCE_STATEMENT_BYTES
        || value.chars().any(char::is_control)
    {
        return Err(EvaluationError::new(
            EvaluationErrorKind::EvidenceTextInvalid,
        ));
    }
    let lower = value.to_ascii_lowercase();
    const INJECTION_PHRASES: &[&str] = &[
        "ignore previous instruction",
        "ignore all previous",
        "jailbreak",
    ];
    // Role labels signal injection only when they open the statement, not mid-prose.
    const INJECTION_PREFIXES: &[&str] = &[
        "system message",
        "developer message",
        "<system",
        "assistant:",
        "system:",
        "developer:",
    ];
    let opening = lower.trim_start_matches(|character: char| {
        character.is_whitespace() || matches!(character, '>' | '#' | '-' | '*' | '"' | '\'' | '`')
    });
    if INJECTION_PHRASES
        .iter()
        .any(|marker| lower.contains(marker))
        || INJECTION_PREFIXES
            .iter()
            .any(|marker| opening.starts_with(marker))
    {
        return Err(EvaluationError::new(EvaluationErrorKind::PromptInjection));
    }
    const SENSITIVE_MARKERS: &[&str] = &[
        "authorization: bearer",
        "api_key=",
        "api-key=",
        "access_token=",
        "refresh_token=",
        "password=",
        "private key-----",
        "-----begin private key",
        "chatgpt oauth",
        "auth.json",
    ];
    if SENSITIVE_MARKERS
        .iter()
        .any(|marker| lower.contains(marker))
        || lower.split_whitespace().any(|part| part.starts_with("sk-"))
    {
        return Err(EvaluationError::new(EvaluationErrorKind::SensitiveContent));
    }
    if lower.lines().any(|line| {
        line.starts_with("diff --git ")
            || line.starts_with("@@ ")
            || line.starts_with("+++ ")
            || line.starts_with("--- ")
    }) {
        return Err(EvaluationError::new(EvaluationErrorKind::RawDiff));
    }
    if contains_absolute_path(value) {
        return Err(EvaluationError::new(EvaluationErrorKind::AbsolutePath));
    }
    const PERSON_MARKERS: &[&str] = &[
        "developer productivity",
        "contributor performance",
        "employee performance",
        "compensation decision",
        "promotion decision",
        "hiring recommendation",
        "individual contributor score",
    ];
    if PERSON_MARKERS.iter().any(|marker| lower.contains(marker)) {
        return Err(EvaluationError::new(
            EvaluationErrorKind::PersonDomainMixing,
        ));
    }
    Ok(())
}

fn contains_absolute_path(value: &str) -> bool {
    value.split_whitespace().any(|token| {
        let token = token.trim_matches(|character: char| {
            matches!(
                character,
                '(' | ')' | '[' | ']' | '{' | '}' | ',' | ';' | '"' | '\''
            )
        });
        (token.starts_with('/') && token[1..].contains('/'))
            || (token.len() >= 3
                && token.as_bytes()[0].is_ascii_alphabetic()
                && token.as_bytes()[1] == b':'
                && matches!(token.as_bytes()[2], b'/' | b'\\'))
    })
}
