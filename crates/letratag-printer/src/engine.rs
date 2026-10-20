use letratag_protocol::{Job, Profile, Raster, Status};
use letratag_transport::Transport;
use std::fs::{self, File, OpenOptions};
use std::io::{Seek, Write};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("input or protocol validation failed")]
    Protocol(#[from] letratag_protocol::Error),
    #[error("transport failed before transmission")]
    SafeTransport,
    #[error("print outcome uncertain; operator inspection required, no automatic retry")]
    Uncertain,
    #[error("queue blocked by unresolved print journal")]
    Blocked,
    #[error("private journal or capture I/O failed")]
    Io(#[from] std::io::Error),
}

/// Conceptual job lifecycle. PAPPL exposes its own job states to IPP
/// clients (pending, processing, completed, canceled, aborted); the states
/// below describe the engine's view of one submitted job and how they map
/// onto the durable journal.
///
/// - Accepted, Validated, Rendering and ReadyForTransmission happen before
///   the journal exists. A failure there rejects the job without touching
///   hardware (PAPPL reports it as aborted) and leaves no journal entry.
/// - TransmissionStarted and OutcomeUncertain share the journal value
///   "inflight": it is written before the first possible write and survives
///   process restarts. Any outcome after that point is uncertain, blocks
///   the queue, and is resolved only by an explicit operator decision.
/// - TransmissionCompleted means every write was accepted by the host.
///   On hardware this is transport completion, NOT proof of a physical
///   label.
/// - ConfirmedPrinted is deliberately unreachable on the LetraTag 200B:
///   the printer reports success even with the lid open (phantom-success
///   finding, docs/evidence/hardware-failure-cases-2026-10-20.md), so no
///   status code proves physical output. Only the mock transport, whose
///   completion is a test double, reaches it (journal value "completed").
/// - FailedBeforeTransmission is the journal value "failed-safe": the
///   transport failed before any write and the job can be resubmitted.
/// - Cancelled: a client cancellation cannot retract an in-flight
///   transmission; the journal stays inflight and the outcome stays
///   uncertain.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobState {
    Accepted,
    Validated,
    Rendering,
    ReadyForTransmission,
    TransmissionStarted,
    TransmissionCompleted,
    ConfirmedPrinted,
    FailedBeforeTransmission,
    OutcomeUncertain,
    Cancelled,
}

pub struct Engine {
    directory: PathBuf,
    capture: bool,
    diagnostics: bool,
    short_command: Option<Vec<u8>>,
    linger: Duration,
    blocked: bool,
    _lock: File,
}

pub fn private_directory(path: &Path) -> std::io::Result<()> {
    if !path.exists() {
        fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(path)?;
    }
    let md = fs::symlink_metadata(path)?;
    if !md.is_dir() || md.permissions().mode() & 0o077 != 0 {
        return Err(std::io::Error::other(
            "directory must be private (mode 0700)",
        ));
    }
    Ok(())
}

impl Engine {
    pub fn new(directory: impl Into<PathBuf>, capture: bool) -> Result<Self, Error> {
        let directory = directory.into();
        private_directory(&directory)?;
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(directory.join("queue.lock"))?;
        // The descriptor owns the advisory lock until Engine is dropped.
        if unsafe {
            libc::flock(
                std::os::fd::AsRawFd::as_raw_fd(&lock),
                libc::LOCK_EX | libc::LOCK_NB,
            )
        } != 0
        {
            return Err(std::io::Error::other("queue already owned by another process").into());
        }
        let mut blocked = false;
        let mut count = 0;
        for entry in fs::read_dir(&directory)? {
            let entry = entry?;
            if entry.path().extension().is_some_and(|e| e == "state") {
                count += 1;
                if count > 4096 || entry.metadata()?.len() > 64 {
                    return Err(Error::Blocked);
                }
                let text = fs::read_to_string(entry.path())?;
                blocked |= !matches!(text.as_str(), "completed\n" | "failed-safe\n");
            }
        }
        Ok(Self {
            directory,
            capture,
            diagnostics: false,
            short_command: None,
            linger: Duration::ZERO,
            blocked,
            _lock: lock,
        })
    }

    /// Opt-in content-free protocol diagnostics on stderr: connection size,
    /// write progress, raw status notification bytes and timing. Never logs
    /// print contents or job titles.
    pub fn with_diagnostics(mut self, enabled: bool) -> Self {
        self.diagnostics = enabled;
        self
    }

    /// Optional short command (e.g. set-cassette-type) sent to the
    /// transport's short-command channel after connect, before the print
    /// job writes. Best effort: a failed short command is logged but does
    /// not stop the job.
    pub fn with_short_command(mut self, command: Option<Vec<u8>>) -> Self {
        self.short_command = command;
        self
    }

    /// Hardware calibration linger: after the status phase, keep the
    /// connection open and drain notifications for this period before
    /// disconnecting. The printer may still be physically printing after a
    /// result-like notification; disconnecting earlier aborts it (observed
    /// 2026-10-20: motor ran, no label produced). Zero disables the linger.
    pub fn with_linger(mut self, linger: Duration) -> Self {
        self.linger = linger;
        self
    }

    pub fn is_blocked(&self) -> bool {
        self.blocked
    }

    pub async fn print(
        &mut self,
        raster: &Raster,
        profile: Profile,
        transport: &mut dyn Transport,
        hardware: bool,
    ) -> Result<(), Error> {
        if self.blocked {
            return Err(Error::Blocked);
        }
        // Bound retained metadata and opt-in captures, never prune an uncertain record.
        if fs::read_dir(&self.directory)?.count() >= 4096 {
            return Err(Error::Blocked);
        }
        let started = Instant::now();
        let diagnostics = self.diagnostics;
        let job = Job::new(raster, profile)?;
        let size = match tokio::time::timeout(Duration::from_secs(12), transport.connect()).await {
            Ok(Ok(size)) => size,
            result => {
                // Transport errors are content-free; the job never started.
                if diagnostics {
                    eprintln!(
                        "[diag] connect failed: {result:?} at {:?}",
                        started.elapsed()
                    );
                }
                let _ = tokio::time::timeout(Duration::from_secs(4), transport.disconnect()).await;
                return Err(Error::SafeTransport);
            }
        };
        if diagnostics {
            eprintln!(
                "[diag] connected; usable value size {size} at {:?}",
                started.elapsed()
            );
            for line in transport.parameters().describe().lines() {
                eprintln!("[diag] {line}");
            }
        }
        if let Some(command) = &self.short_command {
            let result = tokio::time::timeout(
                Duration::from_secs(4),
                transport.write_short_command(command),
            )
            .await;
            if diagnostics {
                eprintln!(
                    "[diag] short command len={} result {result:?} at {:?}",
                    command.len(),
                    started.elapsed()
                );
            }
        }
        let writes = match job.writes(size) {
            Ok(writes) => writes,
            Err(e) => {
                let _ = tokio::time::timeout(Duration::from_secs(4), transport.disconnect()).await;
                return Err(e.into());
            }
        };
        let id = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let mut state = OpenOptions::new()
            .write(true)
            .read(true)
            .create_new(true)
            .mode(0o600)
            .open(self.directory.join(format!("{id}.state")))?;
        set_state(&mut state, b"inflight\n")?;
        File::open(&self.directory)?.sync_all()?;
        self.blocked = true;
        // Any error from this point, including the first write, may have printed.
        let linger = self.linger;
        let outcome = tokio::time::timeout(Duration::from_secs(90), async {
            for (index, write) in writes.iter().enumerate() {
                tokio::time::timeout(Duration::from_secs(4), transport.write(write))
                    .await
                    .map_err(|_| Error::Uncertain)?
                    .map_err(|_| Error::Uncertain)?;
                if diagnostics {
                    eprintln!(
                        "[diag] write {}/{} len={} at {:?}",
                        index + 1,
                        writes.len(),
                        write.len(),
                        started.elapsed()
                    );
                }
            }
            // S1 (ysfchn/dymo-bluetooth) documents a two-message status
            // sequence: a first PRINTING progress notification (code 0 or 1),
            // then the result after physical printing completes. On hardware
            // the first 0/1 notification is therefore never terminal;
            // disconnecting on it aborts an in-progress print (observed
            // 2026-10-20: motor ran, no label produced). The outcome stays
            // uncertain: the terminal sequence is not yet calibrated.
            let mut progress_ack_discarded = false;
            let mut result = Err(Error::Uncertain);
            for _ in 0..16 {
                let reply = tokio::time::timeout(Duration::from_secs(32), transport.status())
                    .await
                    .map_err(|_| Error::Uncertain)?
                    .map_err(|_| Error::Uncertain)?;
                if diagnostics {
                    eprintln!("[diag] status [{}] at {:?}", hex(&reply), started.elapsed());
                }
                match Status::parse(&reply).map_err(|_| Error::Uncertain)? {
                    Status::Success if !hardware => {
                        result = Ok(());
                        break;
                    }
                    Status::Success | Status::Unconfirmed(1)
                        if hardware && !progress_ack_discarded =>
                    {
                        progress_ack_discarded = true;
                        continue;
                    }
                    Status::Unconfirmed(1) => continue,
                    // No hardware completion sequence has been calibrated yet.
                    _ => break,
                }
            }
            // Hardware calibration linger: keep the connection open and drain
            // notifications for the configured period before disconnecting.
            // The printer may still be physically printing after a
            // result-like notification; disconnecting earlier aborts it.
            if hardware && !linger.is_zero() {
                let deadline = Instant::now() + linger;
                while let Some(remaining) = deadline.checked_duration_since(Instant::now()) {
                    let reply = tokio::time::timeout(
                        remaining.min(Duration::from_secs(32)),
                        transport.status(),
                    )
                    .await;
                    match reply {
                        Ok(Ok(bytes)) => {
                            if diagnostics {
                                eprintln!(
                                    "[diag] linger status [{}] at {:?}",
                                    hex(&bytes),
                                    started.elapsed()
                                );
                            }
                        }
                        _ => break,
                    }
                }
            }
            result
        })
        .await
        .unwrap_or(Err(Error::Uncertain));
        if diagnostics {
            eprintln!("[diag] outcome {outcome:?} at {:?}", started.elapsed());
        }
        let disconnected =
            tokio::time::timeout(Duration::from_secs(4), transport.disconnect()).await;
        outcome?;
        if !matches!(disconnected, Ok(Ok(()))) {
            return Err(Error::Uncertain);
        }
        if self.capture {
            let preview = letratag_render::preview_png(raster).map_err(|_| Error::Uncertain)?;
            write_private(&self.directory.join(format!("{id}.png")), &preview)?;
            let mut capture = Vec::new();
            for write in writes {
                capture.extend_from_slice(&(write.len() as u16).to_le_bytes());
                capture.extend_from_slice(&write);
            }
            write_private(&self.directory.join(format!("{id}.ble")), &capture)?;
        }
        set_state(&mut state, b"completed\n")?;
        self.blocked = false;
        Ok(())
    }
}

fn write_private(path: &Path, data: &[u8]) -> std::io::Result<()> {
    let mut f = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)?;
    f.write_all(data)?;
    f.sync_all()
}

fn set_state(file: &mut File, state: &[u8]) -> std::io::Result<()> {
    file.rewind()?;
    file.write_all(state)?;
    file.set_len(state.len() as u64)?;
    file.sync_all()
}

/// Content-free rendering of raw notification bytes (status frames only).
fn hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use letratag_transport::{MockFault, MockTransport};
    fn private_tempdir() -> tempfile::TempDir {
        tempfile::Builder::new()
            .permissions(fs::Permissions::from_mode(0o700))
            .tempdir()
            .unwrap()
    }
    fn raster() -> Raster {
        Raster::new(40, vec![0; 40 * 32]).unwrap()
    }
    #[tokio::test]
    async fn mock_queue_and_capture() {
        let dir = private_tempdir();
        let mut engine = Engine::new(dir.path(), true).unwrap();
        for _ in 0..3 {
            engine
                .print(
                    &raster(),
                    Profile::Modern,
                    &mut MockTransport::default(),
                    false,
                )
                .await
                .unwrap();
        }
        assert_eq!(
            fs::read_dir(dir.path())
                .unwrap()
                .filter(|f| f
                    .as_ref()
                    .unwrap()
                    .path()
                    .extension()
                    .is_some_and(|e| e == "ble"))
                .count(),
            3
        );
    }
    #[tokio::test]
    async fn uncertain_is_never_replayed_and_survives_restart() {
        let dir = private_tempdir();
        let mut engine = Engine::new(dir.path(), false).unwrap();
        let mut transport = MockTransport::with_fault(MockFault::WriteAt(1));
        assert!(matches!(
            engine
                .print(&raster(), Profile::Modern, &mut transport, false)
                .await,
            Err(Error::Uncertain)
        ));
        let mut other = MockTransport::default();
        assert!(matches!(
            engine
                .print(&raster(), Profile::Modern, &mut other, false)
                .await,
            Err(Error::Blocked)
        ));
        assert!(other.writes().is_empty());
        drop(engine);
        assert!(Engine::new(dir.path(), false).unwrap().is_blocked());
    }
    #[tokio::test]
    async fn connect_failure_is_safe_but_status_error_is_not() {
        let dir = private_tempdir();
        let mut engine = Engine::new(dir.path(), false).unwrap();
        assert!(matches!(
            engine
                .print(
                    &raster(),
                    Profile::Modern,
                    &mut MockTransport::with_fault(MockFault::ConnectTimeout),
                    false
                )
                .await,
            Err(Error::SafeTransport)
        ));
        assert!(!engine.is_blocked());
        assert!(matches!(
            engine
                .print(
                    &raster(),
                    Profile::Modern,
                    &mut MockTransport::with_fault(MockFault::StatusCode(7)),
                    false
                )
                .await,
            Err(Error::Uncertain)
        ));
    }
    #[test]
    fn lock_excludes_second_process_owner() {
        let dir = private_tempdir();
        let _engine = Engine::new(dir.path(), false).unwrap();
        assert!(Engine::new(dir.path(), false).is_err());
    }
    #[tokio::test]
    async fn uncalibrated_hardware_policy_never_claims_completion() {
        let dir = private_tempdir();
        let mut engine = Engine::new(dir.path(), false).unwrap();
        let mut transport = MockTransport::default();
        assert!(matches!(
            engine
                .print(&raster(), Profile::Modern, &mut transport, true)
                .await,
            Err(Error::Uncertain)
        ));
        assert!(!transport.writes().is_empty());
        assert!(engine.is_blocked());
    }

    #[tokio::test]
    async fn hardware_discards_only_the_first_progress_ack() {
        // S1's two-message sequence: a first PRINTING notification (code 0
        // or 1) is progress, not the result. The engine must read past it
        // before the status phase can end.
        let dir = private_tempdir();
        let mut engine = Engine::new(dir.path(), false).unwrap();
        let mut transport = MockTransport::with_fault(MockFault::StatusCodes(vec![0, 2]));
        assert!(matches!(
            engine
                .print(&raster(), Profile::Modern, &mut transport, true)
                .await,
            Err(Error::Uncertain)
        ));
        assert_eq!(transport.status_reads(), 2);
        assert!(engine.is_blocked());

        let dir = private_tempdir();
        let mut engine = Engine::new(dir.path(), false).unwrap();
        let mut transport = MockTransport::with_fault(MockFault::StatusCodes(vec![1, 0]));
        assert!(matches!(
            engine
                .print(&raster(), Profile::Modern, &mut transport, true)
                .await,
            Err(Error::Uncertain)
        ));
        assert_eq!(transport.status_reads(), 2);
    }

    #[tokio::test]
    async fn hardware_does_not_mask_an_immediate_error() {
        let dir = private_tempdir();
        let mut engine = Engine::new(dir.path(), false).unwrap();
        let mut transport = MockTransport::with_fault(MockFault::StatusCodes(vec![2]));
        assert!(matches!(
            engine
                .print(&raster(), Profile::Modern, &mut transport, true)
                .await,
            Err(Error::Uncertain)
        ));
        assert_eq!(transport.status_reads(), 1);
    }

    #[tokio::test]
    async fn mock_completion_still_uses_the_first_status() {
        let dir = private_tempdir();
        let mut engine = Engine::new(dir.path(), false).unwrap();
        let mut transport = MockTransport::with_fault(MockFault::StatusCodes(vec![0, 2]));
        engine
            .print(&raster(), Profile::Modern, &mut transport, false)
            .await
            .unwrap();
        assert_eq!(transport.status_reads(), 1);
        assert!(!engine.is_blocked());
    }

    #[tokio::test]
    async fn short_command_is_sent_after_connect_before_the_job() {
        let dir = private_tempdir();
        let mut engine = Engine::new(dir.path(), false)
            .unwrap()
            .with_short_command(Some(vec![1, 2, 3]));
        let mut transport = MockTransport::default();
        engine
            .print(&raster(), Profile::Modern, &mut transport, false)
            .await
            .unwrap();
        assert_eq!(transport.short_commands(), &[vec![1, 2, 3]]);
        assert!(!transport.writes().is_empty());
    }

    #[tokio::test]
    async fn hardware_linger_drains_notifications_before_disconnect() {
        let dir = private_tempdir();
        let mut engine = Engine::new(dir.path(), false)
            .unwrap()
            .with_linger(Duration::from_millis(50));
        let mut transport = MockTransport::default();
        assert!(matches!(
            engine
                .print(&raster(), Profile::Modern, &mut transport, true)
                .await,
            Err(Error::Uncertain)
        ));
        // Two reads end the status phase (progress ack + terminal); the
        // linger drains further notifications before the disconnect.
        assert!(transport.status_reads() > 2);
    }

    #[tokio::test]
    async fn disconnect_failure_after_transmission_is_uncertain() {
        // All writes and the status phase succeeded, but the disconnect
        // failed: the outcome cannot be confirmed, so it stays uncertain
        // and the journal keeps blocking.
        let dir = private_tempdir();
        let mut engine = Engine::new(dir.path(), false).unwrap();
        let mut transport = MockTransport::with_fault(MockFault::Disconnect);
        assert!(matches!(
            engine
                .print(&raster(), Profile::Modern, &mut transport, false)
                .await,
            Err(Error::Uncertain)
        ));
        assert!(!transport.writes().is_empty());
        assert!(engine.is_blocked());
        drop(engine);
        assert!(Engine::new(dir.path(), false).unwrap().is_blocked());
    }

    #[tokio::test(start_paused = true)]
    async fn status_timeout_is_uncertain_and_never_retried() {
        let dir = private_tempdir();
        let mut engine = Engine::new(dir.path(), false).unwrap();
        let mut transport = MockTransport::with_fault(MockFault::StatusTimeout);
        assert!(matches!(
            engine
                .print(&raster(), Profile::Modern, &mut transport, false)
                .await,
            Err(Error::Uncertain)
        ));
        assert!(!transport.writes().is_empty());
        assert_eq!(transport.status_reads(), 0);
        assert!(engine.is_blocked());
    }

    #[tokio::test(start_paused = true)]
    async fn blocked_write_is_uncertain_and_survives_restart() {
        // Cancellation/timeout of an in-flight write: the blocked write is
        // not recorded, the outcome is uncertain, and no replay happens
        // after a restart.
        let dir = private_tempdir();
        let mut engine = Engine::new(dir.path(), false).unwrap();
        let mut transport = MockTransport::with_fault(MockFault::WriteBlock(1));
        assert!(matches!(
            engine
                .print(&raster(), Profile::Modern, &mut transport, true)
                .await,
            Err(Error::Uncertain)
        ));
        assert_eq!(transport.writes().len(), 1); // header only
        assert_eq!(transport.status_reads(), 0);
        assert!(engine.is_blocked());
        drop(engine);
        let mut engine = Engine::new(dir.path(), false).unwrap();
        assert!(engine.is_blocked());
        let mut other = MockTransport::default();
        assert!(matches!(
            engine
                .print(&raster(), Profile::Modern, &mut other, false)
                .await,
            Err(Error::Blocked)
        ));
        assert!(other.writes().is_empty());
    }
}
