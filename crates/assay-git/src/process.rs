use std::{
    ffi::OsStr,
    io::{self, Read},
    path::{Path, PathBuf},
    process::{Command, ExitStatus, Stdio},
    thread,
    time::Instant,
};

use crate::{CollectionError, CollectionErrorKind, CollectionLimits, CollectionStage};

pub(crate) struct GitProcessRunner {
    executable: PathBuf,
    limits: CollectionLimits,
}

impl GitProcessRunner {
    pub(crate) const fn new(executable: PathBuf, limits: CollectionLimits) -> Self {
        Self { executable, limits }
    }

    pub(crate) fn run(
        &self,
        repository: Option<&Path>,
        stage: CollectionStage,
        arguments: &[&OsStr],
        stdout_limit: usize,
    ) -> Result<Vec<u8>, CollectionError> {
        let mut command = Command::new(&self.executable);
        command
            .env_clear()
            .env("GIT_ATTR_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", null_device())
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_FLUSH", "1")
            .env("GIT_NO_LAZY_FETCH", "1")
            .env("GIT_NO_REPLACE_OBJECTS", "1")
            .env("GIT_OPTIONAL_LOCKS", "0")
            .env("GIT_PROTOCOL_FROM_USER", "0")
            .env("GIT_PAGER", "cat")
            .env("GIT_TERMINAL_PROMPT", "0")
            .env("GIT_TRACE", "0")
            .env("GIT_TRACE2", "0")
            .env("GIT_TRACE_PACKET", "0")
            .env("LANG", "C")
            .env("LC_ALL", "C")
            .env("NO_COLOR", "1")
            .env("TZ", "UTC")
            .args(global_arguments())
            .args(arguments)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if let Some(repository) = repository {
            command.current_dir(repository);
        }

        let mut child = command
            .spawn()
            .map_err(|error| CollectionError::new(stage, spawn_error_kind(error.kind())))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| CollectionError::new(stage, CollectionErrorKind::Io))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| CollectionError::new(stage, CollectionErrorKind::Io))?;
        let stdout_thread = thread::spawn(move || drain_bounded(stdout, stdout_limit));
        let stderr_limit = self.limits.max_stderr_bytes;
        let stderr_thread = thread::spawn(move || drain_bounded(stderr, stderr_limit));

        let started = Instant::now();
        let status = loop {
            match child.try_wait() {
                Ok(Some(status)) => break status,
                Ok(None) if started.elapsed() < self.limits.command_timeout => {
                    thread::sleep(std::time::Duration::from_millis(5));
                }
                Ok(None) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    // A hostile replacement for the trusted executable could
                    // leave a grandchild holding inherited pipes. Git helpers
                    // are disabled, and detaching the drainers here keeps the
                    // adapter deadline bounded even in that test condition.
                    drop(stdout_thread);
                    drop(stderr_thread);
                    return Err(CollectionError::new(stage, CollectionErrorKind::Timeout));
                }
                Err(_) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    drop(stdout_thread);
                    drop(stderr_thread);
                    return Err(CollectionError::new(stage, CollectionErrorKind::Io));
                }
            }
        };

        let stdout = stdout_thread
            .join()
            .map_err(|_| CollectionError::new(stage, CollectionErrorKind::Io))?
            .map_err(|_| CollectionError::new(stage, CollectionErrorKind::Io))?;
        let stderr = stderr_thread
            .join()
            .map_err(|_| CollectionError::new(stage, CollectionErrorKind::Io))?
            .map_err(|_| CollectionError::new(stage, CollectionErrorKind::Io))?;
        if stdout.exceeded || stderr.exceeded {
            return Err(CollectionError::new(
                stage,
                CollectionErrorKind::OutputLimit,
            ));
        }
        ensure_success(status, stage)?;
        Ok(stdout.bytes)
    }
}

fn global_arguments() -> impl IntoIterator<Item = &'static OsStr> {
    [
        OsStr::new("--no-pager"),
        OsStr::new("--no-replace-objects"),
        OsStr::new("--no-lazy-fetch"),
        OsStr::new("--literal-pathspecs"),
        OsStr::new("-c"),
        OsStr::new("credential.helper="),
        OsStr::new("-c"),
        OsStr::new(hooks_path_configuration()),
        OsStr::new("-c"),
        OsStr::new("core.fsmonitor=false"),
        OsStr::new("-c"),
        OsStr::new("diff.external="),
        OsStr::new("-c"),
        OsStr::new("diff.trustExitCode=false"),
        OsStr::new("-c"),
        OsStr::new("protocol.allow=never"),
        OsStr::new("-c"),
        OsStr::new("protocol.ext.allow=never"),
        OsStr::new("-c"),
        OsStr::new("protocol.file.allow=never"),
        OsStr::new("-c"),
        OsStr::new("protocol.git.allow=never"),
        OsStr::new("-c"),
        OsStr::new("protocol.http.allow=never"),
        OsStr::new("-c"),
        OsStr::new("protocol.https.allow=never"),
        OsStr::new("-c"),
        OsStr::new("protocol.ssh.allow=never"),
        OsStr::new("-c"),
        OsStr::new("submodule.recurse=false"),
    ]
}

#[cfg(unix)]
fn null_device() -> &'static OsStr {
    OsStr::new("/dev/null")
}

#[cfg(unix)]
const fn hooks_path_configuration() -> &'static str {
    "core.hooksPath=/dev/null"
}

#[cfg(windows)]
const fn hooks_path_configuration() -> &'static str {
    "core.hooksPath=NUL"
}

#[cfg(not(any(unix, windows)))]
const fn hooks_path_configuration() -> &'static str {
    "core.hooksPath="
}

#[cfg(windows)]
fn null_device() -> &'static OsStr {
    OsStr::new("NUL")
}

#[cfg(not(any(unix, windows)))]
fn null_device() -> &'static OsStr {
    OsStr::new("")
}

fn spawn_error_kind(kind: io::ErrorKind) -> CollectionErrorKind {
    match kind {
        io::ErrorKind::NotFound => CollectionErrorKind::ExecutableMissing,
        io::ErrorKind::PermissionDenied => CollectionErrorKind::PermissionDenied,
        _ => CollectionErrorKind::Io,
    }
}

fn ensure_success(status: ExitStatus, stage: CollectionStage) -> Result<(), CollectionError> {
    if status.success() {
        Ok(())
    } else {
        Err(CollectionError::new(
            stage,
            CollectionErrorKind::NonZeroExit,
        ))
    }
}

struct DrainedOutput {
    bytes: Vec<u8>,
    exceeded: bool,
}

fn drain_bounded(mut input: impl Read, limit: usize) -> io::Result<DrainedOutput> {
    let mut bytes = Vec::with_capacity(limit.min(64 * 1024));
    let mut buffer = [0_u8; 8192];
    let mut exceeded = false;
    loop {
        let read = input.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        let remaining = limit.saturating_sub(bytes.len());
        let retained = remaining.min(read);
        bytes.extend_from_slice(&buffer[..retained]);
        exceeded |= retained < read;
    }
    Ok(DrainedOutput { bytes, exceeded })
}
