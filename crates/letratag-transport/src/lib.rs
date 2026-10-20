//! Bounded transports. A successful mock job is not evidence of physical printing.

use async_trait::async_trait;
use std::{future::Future, time::Duration};
use thiserror::Error;

#[cfg(feature = "ble")]
mod bluez;
#[cfg(feature = "ble")]
pub use bluez::{BluezTransport, DiscoveredDevice};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
#[cfg(feature = "ble")]
const WRITE_TIMEOUT: Duration = Duration::from_secs(3);
const STATUS_TIMEOUT: Duration = Duration::from_secs(30);
#[cfg(feature = "ble")]
const DISCONNECT_TIMEOUT: Duration = Duration::from_secs(3);
const MAX_VALUE_SIZE: usize = 512;
const MAX_WRITES: usize = 256;

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
}
