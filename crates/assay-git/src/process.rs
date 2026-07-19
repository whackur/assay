use std::{
    ffi::OsStr,
    io::{self, Read},
    path::{Path, PathBuf},
    process::{Command, ExitStatus, Stdio},
    sync::mpsc::{Receiver, TryRecvError, sync_channel},
    thread,
    time::Instant,
};

use command_group::{CommandGroup, GroupChild};

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

        let mut child = command.group_spawn().map_err(|error| {
            eprintln!("group_spawn error: {error:?}");
            CollectionError::new(stage, spawn_error_kind(error.kind()))
        })?;
        let stdout = child
            .inner()
            .stdout
            .take()
            .ok_or_else(|| CollectionError::new(stage, CollectionErrorKind::Io))?;
        let stderr = child
            .inner()
            .stderr
            .take()
            .ok_or_else(|| CollectionError::new(stage, CollectionErrorKind::Io))?;
        let (stdout_sender, stdout_receiver) = sync_channel(1);
        let stdout_thread = thread::spawn(move || {
            let _ = stdout_sender.send(drain_bounded(stdout, stdout_limit));
        });
        let stderr_limit = self.limits.max_stderr_bytes;
        let (stderr_sender, stderr_receiver) = sync_channel(1);
        let stderr_thread = thread::spawn(move || {
            let _ = stderr_sender.send(drain_bounded(stderr, stderr_limit));
        });

        let started = Instant::now();
        let mut status = None;
        let mut stdout = None;
        let mut stderr = None;
        loop {
            if status.is_none() {
                match child.try_wait() {
                    Ok(value) => status = value,
                    Err(_) => {
                        terminate_group(&mut child);
                        join_drainers(stdout_thread, stderr_thread);
                        return Err(CollectionError::new(stage, CollectionErrorKind::Io));
                    }
                }
            }
            if poll_drain(&stdout_receiver, &mut stdout, stage).is_err()
                || poll_drain(&stderr_receiver, &mut stderr, stage).is_err()
            {
                terminate_group(&mut child);
                join_drainers(stdout_thread, stderr_thread);
                return Err(CollectionError::new(stage, CollectionErrorKind::Io));
            }
            if status.is_some() && stdout.is_some() && stderr.is_some() {
                break;
            }
            if started.elapsed() >= self.limits.command_timeout {
                terminate_group(&mut child);
                join_drainers(stdout_thread, stderr_thread);
                return Err(CollectionError::new(stage, CollectionErrorKind::Timeout));
            }
            thread::sleep(std::time::Duration::from_millis(5));
        }
        join_drainers(stdout_thread, stderr_thread);
        let status = status.expect("the loop exits only with a child status");
        let stdout = stdout
            .expect("the loop exits only after stdout drains")
            .map_err(|_| CollectionError::new(stage, CollectionErrorKind::Io))?;
        let stderr = stderr
            .expect("the loop exits only after stderr drains")
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

fn poll_drain(
    receiver: &Receiver<io::Result<DrainedOutput>>,
    output: &mut Option<io::Result<DrainedOutput>>,
    stage: CollectionStage,
) -> Result<(), CollectionError> {
    if output.is_some() {
        return Ok(());
    }
    match receiver.try_recv() {
        Ok(value) => *output = Some(value),
        Err(TryRecvError::Empty) => {}
        Err(TryRecvError::Disconnected) => {
            return Err(CollectionError::new(stage, CollectionErrorKind::Io));
        }
    }
    Ok(())
}

fn terminate_group(child: &mut GroupChild) {
    let _ = child.kill();
    let _ = child.wait();
}

fn join_drainers(stdout: thread::JoinHandle<()>, stderr: thread::JoinHandle<()>) {
    let _ = stdout.join();
    let _ = stderr.join();
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
