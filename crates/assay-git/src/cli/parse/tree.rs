use crate::{
    CollectionError, CollectionErrorKind, CollectionStage, EntryMode, GitObjectFormat, GitObjectId,
    ObjectKind, RepositoryPath,
};

pub(crate) struct RawTreeEntry {
    pub(crate) path: RepositoryPath,
    pub(crate) mode: EntryMode,
    pub(crate) kind: ObjectKind,
    pub(crate) object_id: GitObjectId,
}

pub(crate) fn parse_tree(
    output: &[u8],
    maximum: usize,
    format: GitObjectFormat,
) -> Result<Vec<RawTreeEntry>, CollectionError> {
    if output.is_empty() {
        return Ok(Vec::new());
    }
    if !output.ends_with(&[0]) {
        return Err(CollectionError::new(
            CollectionStage::EnumerateTree,
            CollectionErrorKind::MalformedOutput,
        ));
    }
    let mut entries = Vec::new();
    let records = &output[..output.len() - 1];
    for record in records.split(|byte| *byte == 0) {
        if entries.len() == maximum {
            return Err(CollectionError::new(
                CollectionStage::EnumerateTree,
                CollectionErrorKind::RecordLimit,
            ));
        }
        let tab = record
            .iter()
            .position(|byte| *byte == b'\t')
            .ok_or_else(|| {
                CollectionError::new(
                    CollectionStage::EnumerateTree,
                    CollectionErrorKind::MalformedOutput,
                )
            })?;
        let header = std::str::from_utf8(&record[..tab]).map_err(|_| {
            CollectionError::new(
                CollectionStage::EnumerateTree,
                CollectionErrorKind::MalformedOutput,
            )
        })?;
        let fields = header.split_ascii_whitespace().collect::<Vec<_>>();
        if fields.len() != 3 {
            return Err(CollectionError::new(
                CollectionStage::EnumerateTree,
                CollectionErrorKind::MalformedOutput,
            ));
        }
        let (mode, expected_kind) = match fields[0] {
            "100644" => (EntryMode::Regular, ObjectKind::Blob),
            "100755" => (EntryMode::Executable, ObjectKind::Blob),
            "120000" => (EntryMode::SymbolicLink, ObjectKind::Blob),
            "160000" => (EntryMode::Gitlink, ObjectKind::Commit),
            _ => {
                return Err(CollectionError::new(
                    CollectionStage::EnumerateTree,
                    CollectionErrorKind::MalformedOutput,
                ));
            }
        };
        let kind = match fields[1] {
            "blob" => ObjectKind::Blob,
            "commit" => ObjectKind::Commit,
            _ => {
                return Err(CollectionError::new(
                    CollectionStage::EnumerateTree,
                    CollectionErrorKind::MalformedOutput,
                ));
            }
        };
        if kind != expected_kind {
            return Err(CollectionError::new(
                CollectionStage::EnumerateTree,
                CollectionErrorKind::MalformedOutput,
            ));
        }
        entries.push(RawTreeEntry {
            path: RepositoryPath::new(record[tab + 1..].to_vec())?,
            mode,
            kind,
            object_id: GitObjectId::parse(
                fields[2].as_bytes(),
                CollectionStage::EnumerateTree,
                format,
            )?,
        });
    }
    if entries.windows(2).any(|pair| pair[0].path >= pair[1].path) {
        return Err(CollectionError::new(
            CollectionStage::EnumerateTree,
            CollectionErrorKind::MalformedOutput,
        ));
    }
    Ok(entries)
}
