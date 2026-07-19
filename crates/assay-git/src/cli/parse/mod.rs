mod diff;
mod tree;
mod version;

pub(crate) use diff::{RawDiffMode, parse_raw_diff};
pub(crate) use tree::parse_tree;
pub(crate) use version::parse_version;

use crate::{CollectionError, CollectionErrorKind, CollectionStage, GitObjectFormat, GitObjectId};

pub(crate) fn single_line(output: &[u8], stage: CollectionStage) -> Result<&[u8], CollectionError> {
    let value = output.strip_suffix(b"\n").unwrap_or(output);
    let value = value.strip_suffix(b"\r").unwrap_or(value);
    if value.is_empty() || value.contains(&b'\n') || value.contains(&b'\r') {
        return Err(CollectionError::new(
            stage,
            CollectionErrorKind::MalformedOutput,
        ));
    }
    Ok(value)
}

pub(crate) fn parse_boolean(
    output: &[u8],
    stage: CollectionStage,
) -> Result<bool, CollectionError> {
    match single_line(output, stage)? {
        b"true" => Ok(true),
        b"false" => Ok(false),
        _ => Err(CollectionError::new(
            stage,
            CollectionErrorKind::MalformedOutput,
        )),
    }
}

pub(crate) fn parse_decimal(output: &[u8], stage: CollectionStage) -> Result<u64, CollectionError> {
    let value = std::str::from_utf8(single_line(output, stage)?)
        .map_err(|_| CollectionError::new(stage, CollectionErrorKind::MalformedOutput))?;
    if value.starts_with('+') || (value.starts_with('0') && value.len() > 1) {
        return Err(CollectionError::new(
            stage,
            CollectionErrorKind::MalformedOutput,
        ));
    }
    value
        .parse::<u64>()
        .map_err(|_| CollectionError::new(stage, CollectionErrorKind::MalformedOutput))
}

pub(crate) fn parse_lines_of_object_ids(
    output: &[u8],
    stage: CollectionStage,
    format: GitObjectFormat,
) -> Result<Vec<GitObjectId>, CollectionError> {
    if output.is_empty() || !output.ends_with(b"\n") {
        return Err(CollectionError::new(
            stage,
            CollectionErrorKind::MalformedOutput,
        ));
    }
    output[..output.len() - 1]
        .split(|byte| *byte == b'\n')
        .map(|line| GitObjectId::parse(line, stage, format))
        .collect()
}

pub(crate) fn parse_first_parent(
    output: &[u8],
    format: GitObjectFormat,
) -> Result<Option<GitObjectId>, CollectionError> {
    let line = single_line(output, CollectionStage::ReadParentDelta)?;
    let fields = line.split(|byte| *byte == b' ').collect::<Vec<_>>();
    if fields.is_empty() {
        return Err(CollectionError::new(
            CollectionStage::ReadParentDelta,
            CollectionErrorKind::MalformedOutput,
        ));
    }
    for field in &fields {
        GitObjectId::parse(field, CollectionStage::ReadParentDelta, format)?;
    }
    if fields.len() == 1 {
        Ok(None)
    } else {
        GitObjectId::parse(fields[1], CollectionStage::ReadParentDelta, format).map(Some)
    }
}
