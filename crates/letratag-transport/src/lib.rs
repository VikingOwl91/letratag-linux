//! Bounded transports. A successful mock job is not evidence of physical printing.
#![doc = include_str!("../README.md")]

use async_trait::async_trait;
use std::{future::Future, time::Duration};
use thiserror::Error;

#[cfg(feature = "ble")]
mod bluez;
#[cfg(feature = "ble")]
pub use bluez::{BluezTransport, DiscoveredDevice, resolve_device};
#[cfg(feature = "btleplug")]
mod btleplug_backend;
#[cfg(feature = "btleplug")]
pub use btleplug_backend::{BtleplugTransport, GattCentral, GattPeripheral, mock};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
#[cfg(any(feature = "ble", feature = "btleplug"))]
const WRITE_TIMEOUT: Duration = Duration::from_secs(3);
/// Bound for a mock write that never completes (MockFault::WriteBlock).
const MOCK_BLOCK_TIMEOUT: Duration = Duration::from_secs(3);
const STATUS_TIMEOUT: Duration = Duration::from_secs(30);
#[cfg(any(feature = "ble", feature = "btleplug"))]
const DISCONNECT_TIMEOUT: Duration = Duration::from_secs(3);
const MAX_VALUE_SIZE: usize = 512;
const MAX_WRITES: usize = 256;

/// Conservative GATT write-value limit, used until a printer capability is
/// hardware-validated. ATT23-safe.
pub const CONSERVATIVE_WRITE_LIMIT: usize = 20;

/// Protocol implementation bound on one GATT write: 500 body bytes plus the
/// 1-byte chunk index and the 2-byte final magic.
pub const PROTOCOL_WRITE_LIMIT: usize = 503;

/// How the effective GATT write-value size is selected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChunkMode {
    /// Largest size permitted by both the transport and a hardware-validated
    /// printer capability; the conservative limit when no capability is
    /// recorded. Never assumes the negotiated MTU proves printer support.
    Auto,
    /// Always the conservative limit, or smaller if the transport requires.
    Safe,
    /// An explicitly configured size, validated against the negotiated
    /// transport limit and the protocol bounds. Values above the validated
    /// printer capability are experimental.
    Manual(usize),
}

/// Transport parameters selected at connect time, for diagnostics. Unknown
/// values are reported explicitly, never invented.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TransportParameters {
    pub transport: Option<&'static str>,
    pub write_mode: Option<&'static str>,
    /// Negotiated ATT MTU as reported by BlueZ (derived: the bluer
    /// mtu_workaround subtracts five).
    pub att_mtu: Option<usize>,
    /// Usable GATT characteristic value size (bluer mtu_workaround applied).
    pub gatt_value_limit: Option<usize>,
    /// Hardware-validated printer write capability, if recorded.
    pub printer_cap: Option<usize>,
    pub effective_write: Option<usize>,
    pub chunk_mode: Option<&'static str>,
    /// True when a manual size exceeds the validated capability (or no
    /// capability is recorded): experimental, not validated.
    pub experimental: bool,
}

impl TransportParameters {
    pub fn unknown() -> Self {
        Self::default()
    }

    /// Human-readable diagnostics block.
    pub fn describe(&self) -> String {
        let unknown = "unknown";
        let cap = match self.printer_cap {
            Some(cap) => cap.to_string(),
            None => unknown.to_string(),
        };
        let effective = match (self.effective_write, self.experimental) {
            (Some(size), true) => format!("{size} (experimental)"),
            (Some(size), false) => size.to_string(),
            (None, _) => unknown.to_string(),
        };
        [
            ("Transport:", self.transport.unwrap_or(unknown)),
            ("Write mode:", self.write_mode.unwrap_or(unknown)),
            (
                "Negotiated ATT MTU:",
                &self.att_mtu.map_or(unknown.to_string(), |v| v.to_string()),
            ),
            (
                "Usable GATT value:",
                &self
                    .gatt_value_limit
                    .map_or(unknown.to_string(), |v| v.to_string()),
            ),
            ("Validated printer cap:", &cap),
            ("Effective write size:", &effective),
            ("Chunk mode:", self.chunk_mode.unwrap_or(unknown)),
        ]
        .iter()
        .map(|(label, value)| format!("{label:<23}{value}"))
        .collect::<Vec<_>>()
        .join("\n")
    }
}

/// effective_write_limit = min(transport, printer capability, protocol).
///
/// The transport limit is the usable GATT value size (never subtract ATT
/// overhead twice: bluer's mtu_workaround is already applied). The printer
/// capability is only used when hardware-validated; an acknowledged ATT
/// write does not prove the printer processed the job.
pub fn effective_write_limit(
    transport_limit: usize,
    printer_cap: Option<usize>,
    mode: ChunkMode,
) -> Result<usize, Error> {
    let selected = match mode {
        ChunkMode::Auto => printer_cap.unwrap_or(CONSERVATIVE_WRITE_LIMIT),
        ChunkMode::Safe => CONSERVATIVE_WRITE_LIMIT,
        ChunkMode::Manual(value) => {
            if !(9..=512).contains(&value) {
                return Err(Error::InvalidWriteSize(value));
            }
            if value > transport_limit {
                return Err(Error::WriteSizeExceedsTransport {
                    configured: value,
                    limit: transport_limit,
                });
            }
            value
        }
    };
    let limit = transport_limit.min(selected).min(PROTOCOL_WRITE_LIMIT);
    if limit < 9 {
        return Err(Error::WriteSizeTooSmall(limit));
    }
    Ok(limit)
}

/// Diagnostics label for the configured chunk mode.
pub(crate) fn chunk_mode_label(mode: ChunkMode) -> &'static str {
    match mode {
        ChunkMode::Auto => "auto",
        ChunkMode::Safe => "safe",
        ChunkMode::Manual(_) => "manual",
    }
}

/// A manual size above the validated capability (or with no recorded
/// capability at all) is experimental, not validated.
pub(crate) fn experimental(mode: ChunkMode, printer_cap: Option<usize>) -> bool {
    match mode {
        ChunkMode::Manual(value) => printer_cap.is_none_or(|cap| value > cap),
        _ => false,
    }
}

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    #[error("invalid device address")]
    InvalidAddress,
    #[error("transport is not in the required state")]
    InvalidState,
    #[error("connection timed out")]
    ConnectTimeout,
    #[error("connection failed")]
    Connect,
    #[error("GATT profile missing or ambiguous")]
    Profile,
    #[error("write timed out; delivery is uncertain")]
    WriteTimeout,
    #[error("write failed; delivery is uncertain")]
    Write,
    #[error("characteristic value exceeds the transport limit")]
    ValueTooLarge,
    #[error("job write limit reached")]
    WriteLimit,
    #[error("status timed out")]
    StatusTimeout,
    #[error("status notification stream ended")]
    StatusEnded,
    #[error("status notification exceeds the transport limit")]
    StatusTooLarge,
    #[error("disconnect timed out")]
    DisconnectTimeout,
    #[error("disconnect failed")]
    Disconnect,
    #[error("discovery failed")]
    Discovery,
    #[error("configured device not found during bounded discovery; is it powered on?")]
    DeviceNotFound,
    #[error("configured write size {0} is outside the protocol bounds 9..=512")]
    InvalidWriteSize(usize),
    #[error("configured write size {configured} exceeds the negotiated GATT value limit {limit}")]
    WriteSizeExceedsTransport { configured: usize, limit: usize },
    #[error("the effective write size {0} is too small for the protocol header")]
    WriteSizeTooSmall(usize),
}

#[async_trait]
pub trait Transport: Send {
    /// Returns the usable characteristic value size in bytes, not the ATT MTU.
    async fn connect(&mut self) -> Result<usize, Error>;
    async fn write(&mut self, data: &[u8]) -> Result<(), Error>;
    /// Returns one raw notification, without interpreting printer completion.
    async fn status(&mut self) -> Result<Vec<u8>, Error>;
    async fn disconnect(&mut self) -> Result<(), Error>;
    /// Best-effort write to the short-command channel (the auxiliary
    /// characteristic), used for session setup such as set-cassette-type.
    /// The default is unsupported.
    async fn write_short_command(&mut self, _data: &[u8]) -> Result<(), Error> {
        Err(Error::InvalidState)
    }
    /// Parameters of the current or last connection, for diagnostics. The
    /// default reports every value unknown (never connected).
    fn parameters(&self) -> TransportParameters {
        TransportParameters::unknown()
    }
}

async fn bounded<T>(
    duration: Duration,
    error: Error,
    operation: impl Future<Output = Result<T, Error>>,
) -> Result<T, Error> {
    tokio::time::timeout(duration, operation)
        .await
        .map_err(|_| error)?
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub enum MockFault {
    #[default]
    None,
    ConnectTimeout,
    /// Zero-based write index, including the header at index zero.
    WriteAt(usize),
    /// Zero-based write index whose write never completes. The write is
    /// marked uncertain before blocking, so cancellation cannot enable a
    /// replay.
    WriteBlock(usize),
    StatusTimeout,
    StatusCode(u8),
    /// Status codes returned in read order; repeats the last code.
    StatusCodes(Vec<u8>),
    Disconnect,
}

#[derive(Debug, Default)]
pub struct MockTransport {
    fault: MockFault,
    connected: bool,
    attempted: bool,
    failed: bool,
    writes: Vec<Vec<u8>>,
    short_commands: Vec<Vec<u8>>,
    status_reads: usize,
}

impl MockTransport {
    pub fn with_fault(fault: MockFault) -> Self {
        Self {
            fault,
            ..Self::default()
        }
    }

    pub fn writes(&self) -> &[Vec<u8>] {
        &self.writes
    }

    pub fn short_commands(&self) -> &[Vec<u8>] {
        &self.short_commands
    }

    pub fn status_reads(&self) -> usize {
        self.status_reads
    }
}

#[async_trait]
impl Transport for MockTransport {
    async fn connect(&mut self) -> Result<usize, Error> {
        if self.connected || self.attempted {
            return Err(Error::InvalidState);
        }
        if self.fault == MockFault::ConnectTimeout {
            return bounded(
                CONNECT_TIMEOUT,
                Error::ConnectTimeout,
                std::future::pending(),
            )
            .await;
        }
        self.connected = true;
        Ok(MAX_VALUE_SIZE)
    }

    async fn write(&mut self, data: &[u8]) -> Result<(), Error> {
        if !self.connected || self.failed {
            return Err(Error::InvalidState);
        }
        if data.len() > MAX_VALUE_SIZE {
            return Err(Error::ValueTooLarge);
        }
        if self.writes.len() >= MAX_WRITES {
            return Err(Error::WriteLimit);
        }
        self.attempted = true;
        if self.fault == MockFault::WriteAt(self.writes.len()) {
            self.failed = true;
            return Err(Error::Write);
        }
        if self.fault == MockFault::WriteBlock(self.writes.len()) {
            self.failed = true;
            return bounded(
                MOCK_BLOCK_TIMEOUT,
                Error::WriteTimeout,
                std::future::pending(),
            )
            .await;
        }
        self.writes.push(data.to_vec());
        Ok(())
    }

    async fn status(&mut self) -> Result<Vec<u8>, Error> {
        if !self.connected || self.failed {
            return Err(Error::InvalidState);
        }
        if self.fault == MockFault::StatusTimeout {
            return bounded(STATUS_TIMEOUT, Error::StatusTimeout, std::future::pending()).await;
        }
        let code = match &self.fault {
            MockFault::StatusCode(code) => *code,
            MockFault::StatusCodes(codes) => {
                let index = self.status_reads.min(codes.len().saturating_sub(1));
                codes.get(index).copied().unwrap_or(0)
            }
            _ => 0,
        };
        self.status_reads += 1;
        Ok(vec![0x1b, 0x52, code])
    }

    async fn write_short_command(&mut self, data: &[u8]) -> Result<(), Error> {
        if !self.connected || self.failed {
            return Err(Error::InvalidState);
        }
        self.short_commands.push(data.to_vec());
        Ok(())
    }

    async fn disconnect(&mut self) -> Result<(), Error> {
        self.connected = false;
        if self.fault == MockFault::Disconnect {
            Err(Error::Disconnect)
        } else {
            Ok(())
        }
    }

    fn parameters(&self) -> TransportParameters {
        let limit = self.connected.then_some(MAX_VALUE_SIZE);
        TransportParameters {
            transport: Some("Mock"),
            write_mode: Some("mock"),
            gatt_value_limit: limit,
            effective_write: limit,
            ..TransportParameters::unknown()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn mock_records_exact_writes_and_raw_status() {
        let mut transport = MockTransport::default();
        assert_eq!(transport.write(b"header").await, Err(Error::InvalidState));
        assert_eq!(transport.connect().await, Ok(512));
        transport.write(b"header").await.unwrap();
        transport.write(&[0, 255]).await.unwrap();
        assert_eq!(transport.writes(), &[b"header".to_vec(), vec![0, 255]]);
        assert_eq!(transport.status().await, Ok(vec![0x1b, 0x52, 0]));
        transport.disconnect().await.unwrap();
        assert_eq!(transport.status().await, Err(Error::InvalidState));
        assert_eq!(transport.connect().await, Err(Error::InvalidState));
    }

    #[tokio::test]
    async fn mock_records_short_commands_after_connect() {
        let mut transport = MockTransport::default();
        assert_eq!(
            transport.write_short_command(b"cmd").await,
            Err(Error::InvalidState)
        );
        transport.connect().await.unwrap();
        transport.write_short_command(b"cmd").await.unwrap();
        assert_eq!(transport.short_commands(), &[b"cmd".to_vec()]);
    }

    #[tokio::test(start_paused = true)]
    async fn mock_timeouts_are_bounded() {
        let mut connect = MockTransport::with_fault(MockFault::ConnectTimeout);
        assert_eq!(connect.connect().await, Err(Error::ConnectTimeout));
        let mut status = MockTransport::with_fault(MockFault::StatusTimeout);
        status.connect().await.unwrap();
        assert_eq!(status.status().await, Err(Error::StatusTimeout));
        status.disconnect().await.unwrap();
    }

    #[tokio::test]
    async fn failing_header_is_index_zero_and_never_retried() {
        for index in [0, 1] {
            let mut transport = MockTransport::with_fault(MockFault::WriteAt(index));
            transport.connect().await.unwrap();
            for _ in 0..index {
                transport.write(b"header").await.unwrap();
            }
            assert_eq!(transport.write(b"payload").await, Err(Error::Write));
            assert_eq!(transport.writes().len(), index);
            assert_eq!(transport.write(b"payload").await, Err(Error::InvalidState));
            transport.disconnect().await.unwrap();
            assert_eq!(transport.connect().await, Err(Error::InvalidState));
        }
    }

    #[tokio::test]
    async fn nonzero_status_is_not_interpreted() {
        for code in [1, 2, 255] {
            let mut transport = MockTransport::with_fault(MockFault::StatusCode(code));
            transport.connect().await.unwrap();
            assert_eq!(transport.status().await, Ok(vec![0x1b, 0x52, code]));
        }
    }

    #[tokio::test]
    async fn disconnect_failure_still_closes_mock() {
        let mut transport = MockTransport::with_fault(MockFault::Disconnect);
        transport.connect().await.unwrap();
        assert_eq!(transport.disconnect().await, Err(Error::Disconnect));
        assert_eq!(transport.write(b"x").await, Err(Error::InvalidState));
    }

    #[tokio::test]
    async fn mock_storage_is_bounded_and_invalid_values_not_recorded() {
        let mut transport = MockTransport::default();
        transport.connect().await.unwrap();
        assert_eq!(transport.write(&[0; 513]).await, Err(Error::ValueTooLarge));
        for _ in 0..256 {
            transport.write(&[0; 512]).await.unwrap();
        }
        assert_eq!(transport.write(b"x").await, Err(Error::WriteLimit));
        assert_eq!(transport.writes().len(), 256);
        drop(transport);
    }

    #[tokio::test]
    async fn reconnect_before_any_send_is_explicitly_allowed() {
        let mut transport = MockTransport::default();
        transport.connect().await.unwrap();
        transport.disconnect().await.unwrap();
        transport.connect().await.unwrap();
    }

    #[tokio::test(start_paused = true)]
    async fn dropping_a_pending_connect_records_nothing() {
        let mut transport = MockTransport::with_fault(MockFault::ConnectTimeout);
        assert!(
            tokio::time::timeout(Duration::from_millis(1), transport.connect())
                .await
                .is_err()
        );
        assert!(transport.writes().is_empty());
        assert_eq!(transport.write(b"x").await, Err(Error::InvalidState));
        transport.disconnect().await.unwrap();
        drop(transport);
    }

    #[test]
    fn effective_write_limit_selects_the_minimum_applicable_bound() {
        // Auto without a validated capability falls back to the
        // conservative ATT23-safe limit; the negotiated MTU never promotes.
        assert_eq!(
            effective_write_limit(495, None, ChunkMode::Auto),
            Ok(CONSERVATIVE_WRITE_LIMIT)
        );
        // Auto uses the validated capability when it is the smallest bound.
        assert_eq!(
            effective_write_limit(495, Some(244), ChunkMode::Auto),
            Ok(244)
        );
        // The negotiated transport limit caps the validated capability.
        assert_eq!(
            effective_write_limit(100, Some(244), ChunkMode::Auto),
            Ok(100)
        );
        // The protocol implementation bound caps everything.
        assert_eq!(
            effective_write_limit(600, Some(600), ChunkMode::Auto),
            Ok(PROTOCOL_WRITE_LIMIT)
        );
        // Safe is the conservative limit, or smaller when the transport
        // requires it.
        assert_eq!(
            effective_write_limit(495, Some(244), ChunkMode::Safe),
            Ok(CONSERVATIVE_WRITE_LIMIT)
        );
        assert_eq!(effective_write_limit(12, None, ChunkMode::Safe), Ok(12));
        // Manual is validated against the negotiated transport limit.
        assert_eq!(
            effective_write_limit(495, Some(244), ChunkMode::Manual(128)),
            Ok(128)
        );
        assert_eq!(
            effective_write_limit(495, Some(244), ChunkMode::Manual(244)),
            Ok(244)
        );
        assert_eq!(
            effective_write_limit(100, None, ChunkMode::Manual(128)),
            Err(Error::WriteSizeExceedsTransport {
                configured: 128,
                limit: 100
            })
        );
        for invalid in [0, 1, 8, 513, usize::MAX] {
            assert_eq!(
                effective_write_limit(495, None, ChunkMode::Manual(invalid)),
                Err(Error::InvalidWriteSize(invalid))
            );
        }
        // A transport limit below the protocol header minimum is rejected.
        assert_eq!(
            effective_write_limit(8, None, ChunkMode::Auto),
            Err(Error::WriteSizeTooSmall(8))
        );
        assert_eq!(
            effective_write_limit(0, None, ChunkMode::Safe),
            Err(Error::WriteSizeTooSmall(0))
        );
    }

    #[test]
    fn parameters_describe_reports_unknowns_and_experimental_explicitly() {
        fn assert_field(text: &str, label: &str, value: &str) {
            let line = text
                .lines()
                .find(|line| line.starts_with(label))
                .unwrap_or_else(|| panic!("missing field {label}"));
            assert_eq!(
                line[label.len()..].trim(),
                value,
                "unexpected value for {label}"
            );
        }

        let unknown = TransportParameters::unknown().describe();
        assert_eq!(unknown.lines().count(), 7);
        for label in [
            "Transport:",
            "Write mode:",
            "Negotiated ATT MTU:",
            "Usable GATT value:",
            "Validated printer cap:",
            "Effective write size:",
            "Chunk mode:",
        ] {
            assert_field(&unknown, label, "unknown");
        }

        let bluez = TransportParameters {
            transport: Some("BlueZ"),
            write_mode: Some("Request"),
            att_mtu: None,
            gatt_value_limit: Some(495),
            printer_cap: Some(244),
            effective_write: Some(244),
            chunk_mode: Some("auto"),
            experimental: false,
        };
        let text = bluez.describe();
        assert_field(&text, "Transport:", "BlueZ");
        assert_field(&text, "Write mode:", "Request");
        // BlueZ's MTU property is a characteristic capability, not the
        // negotiated link MTU: reported unknown, never invented.
        assert_field(&text, "Negotiated ATT MTU:", "unknown");
        assert_field(&text, "Usable GATT value:", "495");
        assert_field(&text, "Validated printer cap:", "244");
        assert_field(&text, "Effective write size:", "244");
        assert_field(&text, "Chunk mode:", "auto");

        let mut experimental = bluez.clone();
        experimental.effective_write = Some(400);
        experimental.experimental = true;
        experimental.chunk_mode = Some("manual");
        let text = experimental.describe();
        assert_field(&text, "Effective write size:", "400 (experimental)");
        assert_field(&text, "Chunk mode:", "manual");
    }

    #[tokio::test]
    async fn mock_reports_its_parameters_after_connect() {
        let mut transport = MockTransport::default();
        let before = transport.parameters();
        assert_eq!(before.transport, Some("Mock"));
        assert_eq!(before.gatt_value_limit, None);
        assert_eq!(before.effective_write, None);
        transport.connect().await.unwrap();
        let after = transport.parameters();
        assert_eq!(after.gatt_value_limit, Some(MAX_VALUE_SIZE));
        assert_eq!(after.effective_write, Some(MAX_VALUE_SIZE));
    }

    #[tokio::test(start_paused = true)]
    async fn blocked_write_times_out_and_is_never_retried() {
        let mut transport = MockTransport::with_fault(MockFault::WriteBlock(1));
        transport.connect().await.unwrap();
        transport.write(b"header").await.unwrap();
        assert_eq!(transport.write(b"payload").await, Err(Error::WriteTimeout));
        assert_eq!(transport.writes().len(), 1);
        // The blocked write is not recorded and the transport stays failed:
        // a later write is rejected, never replayed.
        assert_eq!(transport.write(b"payload").await, Err(Error::InvalidState));
        transport.disconnect().await.unwrap();
    }
}
