use crate::{CollectionError, CollectionErrorKind, CollectionStage, GitObjectFormat};

use super::super::error::malformed_parent_delta;

pub(crate) struct RawChange {
    pub(crate) renamed: bool,
}

#[derive(Clone, Copy)]
pub(crate) enum RawDiffMode {
    NoRenames,
    FindRenames,
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum DiffModeClass {
    Absent,
    RegularBlob,
    Symlink,
    Gitlink,
}

pub(crate) fn parse_raw_diff(
    output: &[u8],
    format: GitObjectFormat,
    mode: RawDiffMode,
) -> Result<Vec<RawChange>, CollectionError> {
    if output.is_empty() {
        return Ok(Vec::new());
    }
    if !output.ends_with(&[0]) {
        return Err(CollectionError::new(
            CollectionStage::ReadParentDelta,
            CollectionErrorKind::MalformedOutput,
        ));
    }
    let segments = output[..output.len() - 1]
        .split(|byte| *byte == 0)
        .collect::<Vec<_>>();
    let mut index = 0;
    let mut changes = Vec::new();
    while index < segments.len() {
        let header = segments[index];
        index += 1;
        let fields = header.split(|byte| *byte == b' ').collect::<Vec<_>>();
        if fields.len() != 5
            || !valid_diff_object_id(fields[2], format)
            || !valid_diff_object_id(fields[3], format)
        {
            return Err(malformed_parent_delta());
        }
        let old_mode = parse_diff_mode(fields[0].strip_prefix(b":").unwrap_or_default())
            .ok_or_else(malformed_parent_delta)?;
        let new_mode = parse_diff_mode(fields[1]).ok_or_else(malformed_parent_delta)?;
        let status = parse_diff_status(&fields, old_mode, new_mode, mode)?;
        if index >= segments.len() || segments[index].is_empty() {
            return Err(CollectionError::new(
                CollectionStage::ReadParentDelta,
                CollectionErrorKind::MalformedOutput,
            ));
        }
        index += 1;
        let renamed = status == b'R';
        if renamed {
            if index >= segments.len() || segments[index].is_empty() {
                return Err(CollectionError::new(
                    CollectionStage::ReadParentDelta,
                    CollectionErrorKind::MalformedOutput,
                ));
            }
            index += 1;
        }
        changes.push(RawChange { renamed });
    }
    Ok(changes)
}

fn parse_diff_mode(value: &[u8]) -> Option<DiffModeClass> {
    match value {
        b"000000" => Some(DiffModeClass::Absent),
        b"100644" | b"100755" => Some(DiffModeClass::RegularBlob),
        b"120000" => Some(DiffModeClass::Symlink),
        b"160000" => Some(DiffModeClass::Gitlink),
        _ => None,
    }
}

fn valid_diff_object_id(value: &[u8], format: GitObjectFormat) -> bool {
    value.len() == format.identifier_length()
        && value
            .iter()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}

fn parse_diff_status(
    fields: &[&[u8]],
    old_mode: DiffModeClass,
    new_mode: DiffModeClass,
    mode: RawDiffMode,
) -> Result<u8, CollectionError> {
    let old_null = fields[2].iter().all(|byte| *byte == b'0');
    let new_null = fields[3].iter().all(|byte| *byte == b'0');
    let old_absent = old_mode == DiffModeClass::Absent;
    let new_absent = new_mode == DiffModeClass::Absent;
    let status = fields[4];
    let valid = match status {
        [b'A'] => old_absent && old_null && !new_absent && !new_null,
        [b'D'] => !old_absent && !old_null && new_absent && new_null,
        [b'M'] => !old_absent && !old_null && !new_absent && !new_null && old_mode == new_mode,
        [b'T'] => !old_absent && !old_null && !new_absent && !new_null && old_mode != new_mode,
        [b'R', hundreds, tens, ones] if matches!(mode, RawDiffMode::FindRenames) => {
            !old_absent
                && !old_null
                && !new_absent
                && !new_null
                && old_mode == new_mode
                && valid_rename_score(*hundreds, *tens, *ones)
        }
        _ => false,
    };
    if valid {
        Ok(status[0])
    } else {
        Err(malformed_parent_delta())
    }
}

fn valid_rename_score(hundreds: u8, tens: u8, ones: u8) -> bool {
    let valid_digits = hundreds.is_ascii_digit() && tens.is_ascii_digit() && ones.is_ascii_digit();
    if !valid_digits {
        return false;
    }
    let score = usize::from(hundreds - b'0') * 100
        + usize::from(tens - b'0') * 10
        + usize::from(ones - b'0');
    (50..=100).contains(&score)
}
