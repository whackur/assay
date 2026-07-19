use std::{fs, io::Write, path::Path};

use serde_json::Value;
use tempfile::NamedTempFile;

use crate::errors::{RunError, output_serialization_failed};

pub(crate) fn json_bytes(value: &Value) -> Result<Vec<u8>, RunError> {
    let mut bytes = serde_json::to_vec_pretty(value).map_err(|_| output_serialization_failed())?;
    bytes.push(b'\n');
    Ok(bytes)
}

pub(crate) fn write_output(
    bytes: &[u8],
    destination: &Path,
    stdout: &mut dyn Write,
) -> Result<(), &'static str> {
    if destination == Path::new("-") {
        stdout.write_all(bytes).map_err(|_| "stdout_write")?;
        stdout.flush().map_err(|_| "stdout_flush")?;
        return Ok(());
    }
    if fs::symlink_metadata(destination).is_ok() {
        return Err("destination_exists");
    }
    let parent = destination
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let mut temporary = NamedTempFile::new_in(parent).map_err(|_| "temporary_create")?;
    temporary.write_all(bytes).map_err(|_| "temporary_write")?;
    temporary
        .as_file_mut()
        .sync_all()
        .map_err(|_| "temporary_sync")?;
    temporary
        .persist_noclobber(destination)
        .map_err(|_| "atomic_persist")?;
    Ok(())
}
