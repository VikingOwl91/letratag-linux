//! `btleplug` BLE transport backend (Linux: BlueZ via bluez-async/D-Bus).
//!
//! This backend mirrors [`super::bluez::BluezTransport`] operation-for-
//! operation: bounded discovery with exact-address selection, one validated
//! vendor GATT service, Write Request (or the characteristic's command mode
//! when it offers no acknowledged write), sequential awaited writes with
//! per-write timeouts, RX notification status reads, and conservative
//! uncertain outcomes. The GATT operations sit behind the public
//! [`GattCentral`]/[`GattPeripheral`] boundary so transport behavior is
//! deterministically testable without hardware; the [`mock`] module provides
//! a scripted implementation.
//!
//! btleplug exposes no negotiated-MTU or GATT-value-size API on Linux, so
//! the ATT MTU and GATT value limit are reported as unknown and the
//! protocol write bound (503) is the only transport-side write limit. The
//! hardware-validated printer cap (244 for the reference device) remains
//! the effective limit; a successful write never proves physical printing.

use super::*;
use async_trait::async_trait;
use btleplug::api::{
    Central as _, CharPropFlags, Characteristic, Manager as _, Peripheral as _, ScanFilter,
    ValueNotification, WriteType,
};
use btleplug::platform;
use futures_util::{Stream, StreamExt};
use std::collections::VecDeque;
use std::pin::Pin;
use std::time::Duration;
use uuid::Uuid;

/// Bounded discovery wait when the adapter does not know the configured
/// device. Matches the BlueZ backend (2026-10-21 lifecycle finding).
const DISCOVERY_TIMEOUT: Duration = Duration::from_secs(15);
/// Bound on inspected discovery polls, mirroring the BlueZ backend.
const MAX_DISCOVERY_EVENTS: usize = 256;
/// Scan poll interval while waiting for the configured device.
const DISCOVERY_POLL: Duration = Duration::from_millis(100);
/// Transport-side write bound: the protocol limit (500 body bytes plus the
/// 1-byte chunk index and 2-byte final magic) is the only write size btleplug
/// does not report on Linux. The validated printer cap is still the effective
/// limit for real jobs.
const TRANSPORT_WRITE_LIMIT: usize = PROTOCOL_WRITE_LIMIT;

type NotificationStream = Pin<Box<dyn Stream<Item = ValueNotification> + Send>>;

fn is_service(uuid: &Uuid) -> bool {
    uuid.as_u128() >> 96 == 0xbe3d_d650
}

fn tail(service: &Uuid, n: u128) -> Uuid {
    Uuid::from_u128(service.as_u128() + (n << 96))
}

/// BLE adapter/scanning boundary: finding the configured device by exact
/// address. Identity is ultimately proven by the GATT profile validation,
/// never by the advertised name.
#[async_trait]
pub trait GattCentral: Send {
    /// Human-readable adapter identity for diagnostics.
    async fn adapter_info(&self) -> Result<String, Error>;
    /// Scan (bounded) until the exact device address appears, then stop
    /// scanning and return the connected-handle factory. `DeviceNotFound`
    /// when the bound expires; `Discovery` when scanning itself fails.
    async fn find(
        &mut self,
        address: &str,
        timeout: Duration,
        max_events: usize,
    ) -> Result<Box<dyn GattPeripheral>, Error>;
}

/// GATT operation boundary for one connected peripheral. Async methods take
/// `&mut self` so the returned futures stay `Send` without requiring the
/// handle to be `Sync` (the notification stream is `Send` but not `Sync`).
#[async_trait]
pub trait GattPeripheral: Send {
    /// Establish the link. Errors map to `Error::Connect`.
    async fn connect(&mut self) -> Result<(), Error>;
    /// True when the link is currently up.
    async fn is_connected(&mut self) -> Result<bool, Error>;
    /// Release the link. Errors map to `Error::Disconnect`.
    async fn disconnect(&mut self) -> Result<(), Error>;
    /// Populate the service/characteristic table. Errors map to
    /// `Error::Profile`.
    async fn discover_services(&mut self) -> Result<(), Error>;
    /// The discovered characteristics (empty before discovery).
    fn characteristics(&self) -> Vec<Characteristic>;
    /// One GATT write. `write_type` selects ATT Write Request
    /// (`WithResponse`) or Write Command (`WithoutResponse`).
    async fn write(
        &mut self,
        characteristic: &Characteristic,
        data: &[u8],
        write_type: WriteType,
    ) -> Result<(), Error>;
    /// Read a characteristic value (status reads; never print data).
    async fn read(&mut self, characteristic: &Characteristic) -> Result<Vec<u8>, Error>;
    /// Subscribe to a characteristic and start its notification stream.
    async fn subscribe(&mut self, characteristic: &Characteristic) -> Result<(), Error>;
    /// Next device notification. The transport filters by the RX
    /// characteristic, like the BlueZ backend's per-characteristic stream.
    /// `Ok(None)` when the stream ends.
    async fn next_notification(&mut self) -> Result<Option<ValueNotification>, Error>;
    /// Advertised local name, if any (a discovery hint, never identity).
    async fn local_name(&mut self) -> Result<Option<String>, Error>;
}

/// Real btleplug adapter boundary.
struct BtleplugCentral {
    adapter: platform::Adapter,
}

impl BtleplugCentral {
    async fn new() -> Result<Self, Error> {
        let manager = platform::Manager::new().await.map_err(|_| Error::Connect)?;
        let mut adapters = manager.adapters().await.map_err(|_| Error::Connect)?;
        let adapter = adapters.drain(..).next().ok_or(Error::Connect)?;
        Ok(Self { adapter })
    }
}

#[async_trait]
impl GattCentral for BtleplugCentral {
    async fn adapter_info(&self) -> Result<String, Error> {
        self.adapter
            .adapter_info()
            .await
            .map_err(|_| Error::Discovery)
    }

    async fn find(
        &mut self,
        address: &str,
        timeout: Duration,
        max_events: usize,
    ) -> Result<Box<dyn GattPeripheral>, Error> {
        let target: btleplug::api::BDAddr = address.parse().map_err(|_| Error::InvalidAddress)?;
        let scan = async {
            // No service filter: selection is the exact address; the GATT
            // profile validation after connect proves identity.
            self.adapter
                .start_scan(ScanFilter::default())
                .await
                .map_err(|_| Error::Discovery)?;
            let found = tokio::time::timeout(timeout, async {
                for _ in 0..max_events {
                    if let Ok(peripherals) = self.adapter.peripherals().await
                        && let Some(peripheral) =
                            peripherals.into_iter().find(|p| p.address() == target)
                    {
                        return Some(peripheral);
                    }
                    tokio::time::sleep(DISCOVERY_POLL).await;
                }
                None
            })
            .await;
            let _ = self.adapter.stop_scan().await;
            Ok::<_, Error>(found.ok().flatten())
        };
        match scan.await {
            Ok(Some(peripheral)) => Ok(Box::new(BtleplugPeripheralHandle {
                peripheral,
                notifications: None,
            })),
            Ok(None) | Err(_) => Err(Error::DeviceNotFound),
        }
    }
}

/// Real btleplug peripheral handle.
struct BtleplugPeripheralHandle {
    peripheral: platform::Peripheral,
    notifications: Option<NotificationStream>,
}

#[async_trait]
impl GattPeripheral for BtleplugPeripheralHandle {
    async fn connect(&mut self) -> Result<(), Error> {
        // Reuse an existing link instead of failing (same rationale as the
        // BlueZ backend: the printer is a single-central peripheral).
        if !self
            .peripheral
            .is_connected()
            .await
            .map_err(|_| Error::Connect)?
        {
            self.peripheral
                .connect()
                .await
                .map_err(|_| Error::Connect)?;
        }
        Ok(())
    }

    async fn is_connected(&mut self) -> Result<bool, Error> {
        self.peripheral
            .is_connected()
            .await
            .map_err(|_| Error::Connect)
    }

    async fn disconnect(&mut self) -> Result<(), Error> {
        self.peripheral
            .disconnect()
            .await
            .map_err(|_| Error::Disconnect)
    }

    async fn discover_services(&mut self) -> Result<(), Error> {
        self.peripheral
            .discover_services()
            .await
            .map_err(|_| Error::Profile)
    }

    fn characteristics(&self) -> Vec<Characteristic> {
        self.peripheral.characteristics().into_iter().collect()
    }

    async fn write(
        &mut self,
        characteristic: &Characteristic,
        data: &[u8],
        write_type: WriteType,
    ) -> Result<(), Error> {
        self.peripheral
            .write(characteristic, data, write_type)
            .await
            .map_err(|_| Error::Write)
    }

    async fn read(&mut self, characteristic: &Characteristic) -> Result<Vec<u8>, Error> {
        self.peripheral
            .read(characteristic)
            .await
            .map_err(|_| Error::Connect)
    }

    async fn subscribe(&mut self, characteristic: &Characteristic) -> Result<(), Error> {
        self.peripheral
            .subscribe(characteristic)
            .await
            .map_err(|_| Error::Connect)?;
        self.notifications = Some(
            self.peripheral
                .notifications()
                .await
                .map_err(|_| Error::Connect)?,
        );
        Ok(())
    }

    async fn next_notification(&mut self) -> Result<Option<ValueNotification>, Error> {
        let Some(notifications) = self.notifications.as_mut() else {
            return Err(Error::InvalidState);
        };
        Ok(notifications.next().await)
    }

    async fn local_name(&mut self) -> Result<Option<String>, Error> {
        Ok(self
            .peripheral
            .properties()
            .await
            .map_err(|_| Error::Discovery)?
            .and_then(|properties| properties.local_name))
    }
}

/// How print data is written to the TX characteristic. Mirrors the BlueZ
/// backend's mode selection from characteristic properties.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TxMode {
    /// ATT Write Request (WithResponse), acknowledged by the device.
    Request,
    /// ATT Write Command (WithoutResponse), unacknowledged.
    Command,
}

impl TxMode {
    fn write_type(self) -> WriteType {
        match self {
            TxMode::Request => WriteType::WithResponse,
            TxMode::Command => WriteType::WithoutResponse,
        }
    }

    fn label(self) -> &'static str {
        match self {
            TxMode::Request => "Request",
            TxMode::Command => "Command",
        }
    }
}

/// Resolved GATT profile: the TX/RX/AUX characteristics of the single vendor
/// service, plus the selected write mode. Validation never guesses: a
/// missing or ambiguous profile is an error.
#[derive(Debug)]
struct GattProfile {
    tx: Characteristic,
    rx: Characteristic,
    aux: Option<Characteristic>,
    mode: TxMode,
}

fn resolve_profile(characteristics: &[Characteristic]) -> Result<GattProfile, Error> {
    let mut services: Vec<Uuid> = characteristics.iter().map(|c| c.service_uuid).collect();
    services.sort();
    services.dedup();
    let vendor: Vec<Uuid> = services.into_iter().filter(is_service).collect();
    if vendor.len() != 1 {
        return Err(Error::Profile);
    }
    let service = vendor[0];
    let in_service = |uuid: Uuid| {
        characteristics
            .iter()
            .find(|c| c.service_uuid == service && c.uuid == uuid)
            .cloned()
    };
    let find = |n: u128| in_service(tail(&service, n));
    // Characteristic flags: (write, write-without-response, notify).
    let flags = |c: &Characteristic| {
        (
            c.properties.contains(CharPropFlags::WRITE),
            c.properties.contains(CharPropFlags::WRITE_WITHOUT_RESPONSE),
            c.properties.contains(CharPropFlags::NOTIFY),
        )
    };
    let Some(tx) = find(1) else {
        return Err(Error::Profile);
    };
    let (write, without_response, _) = flags(&tx);
    if !write && !without_response {
        return Err(Error::Profile);
    }
    // Same preference as the BlueZ backend: an unacknowledged command when the
    // characteristic offers it, otherwise an acknowledged request.
    let mode = if without_response {
        TxMode::Command
    } else {
        TxMode::Request
    };
    let Some(rx) = find(2) else {
        return Err(Error::Profile);
    };
    if !flags(&rx).2 {
        return Err(Error::Profile);
    }
    // Optional auxiliary short-command channel (first writable match).
    let aux = find(3).filter(|c| {
        let (write, without_response, _) = flags(c);
        write || without_response
    });
    Ok(GattProfile { tx, rx, aux, mode })
}

/// btleplug-backed transport. Construction never contacts D-Bus; the central
/// is created (or injected, for tests) at connect time.
pub struct BtleplugTransport {
    address: String,
    central: Option<Box<dyn GattCentral>>,
    peripheral: Option<Box<dyn GattPeripheral>>,
    profile: Option<GattProfile>,
    adapter_info: Option<String>,
    usable: usize,
    chunk_mode: ChunkMode,
    printer_cap: Option<usize>,
    parameters: TransportParameters,
    attempted: bool,
    writes: usize,
    failed: bool,
}

impl BtleplugTransport {
    pub fn new(address: &str) -> Result<Self, Error> {
        // Validate eagerly, like the BlueZ backend.
        address
            .parse::<btleplug::api::BDAddr>()
            .map_err(|_| Error::InvalidAddress)?;
        Ok(Self {
            address: address.to_string(),
            central: None,
            peripheral: None,
            profile: None,
            adapter_info: None,
            usable: 0,
            chunk_mode: ChunkMode::Auto,
            printer_cap: None,
            parameters: TransportParameters::unknown(),
            attempted: false,
            writes: 0,
            failed: false,
        })
    }

    /// Inject a GATT central (tests and diagnostics). The default creates a
    /// real btleplug central at connect time.
    pub fn with_central(mut self, central: Box<dyn GattCentral>) -> Self {
        self.central = Some(central);
        self
    }

    /// Select how the print-chunk write size is chosen at connect time.
    /// Default: Auto.
    pub fn with_chunk_mode(mut self, mode: ChunkMode) -> Self {
        self.chunk_mode = mode;
        self
    }

    /// Record a hardware-validated printer write capability in bytes per
    /// GATT value. Only operator-confirmed limits belong here.
    pub fn with_printer_cap(mut self, cap: Option<usize>) -> Self {
        self.printer_cap = cap;
        self
    }

    async fn open(&mut self) -> Result<usize, Error> {
        let mut central = match self.central.take() {
            Some(central) => central,
            None => Box::new(BtleplugCentral::new().await?),
        };
        let adapter_info = central
            .adapter_info()
            .await
            .unwrap_or_else(|_| "unknown".into());
        let mut peripheral = central
            .find(&self.address, DISCOVERY_TIMEOUT, MAX_DISCOVERY_EVENTS)
            .await?;
        let name = peripheral.local_name().await.unwrap_or(None);
        self.adapter_info = Some(format!(
            "{}; device {:?}",
            adapter_info,
            name.unwrap_or_else(|| "(unnamed)".to_string())
        ));
        // Retain the handle before connecting so explicit cleanup also works
        // after a timeout or partially resolved profile.
        peripheral.connect().await?;
        peripheral.discover_services().await?;
        let profile = resolve_profile(&peripheral.characteristics())?;
        // Subscribe before the first send, like the BlueZ backend.
        peripheral.subscribe(&profile.rx).await?;
        // The effective write size is the minimum of the transport bound, the
        // validated printer capability and the protocol bound. btleplug does
        // not report a GATT value limit on Linux, so the protocol bound is
        // the transport side; the negotiated ATT MTU stays unknown.
        let usable =
            effective_write_limit(TRANSPORT_WRITE_LIMIT, self.printer_cap, self.chunk_mode)?;
        self.parameters = TransportParameters {
            transport: Some("btleplug"),
            write_mode: Some(profile.mode.label()),
            att_mtu: None,
            gatt_value_limit: None,
            printer_cap: self.printer_cap,
            effective_write: Some(usable),
            chunk_mode: Some(chunk_mode_label(self.chunk_mode)),
            experimental: experimental(self.chunk_mode, self.printer_cap),
        };
        self.profile = Some(profile);
        self.usable = usable;
        self.peripheral = Some(peripheral);
        Ok(usable)
    }

    /// Formatted GATT table for backend comparison diagnostics. `None` when
    /// not connected.
    pub fn describe_gatt(&self) -> Option<String> {
        let peripheral = self.peripheral.as_ref()?;
        let characteristics = peripheral.characteristics();
        if characteristics.is_empty() {
            return None;
        }
        let mut lines = vec![format!(
            "btleplug GATT table for {} on {} ({} characteristics)",
            self.address,
            self.adapter_info.as_deref().unwrap_or("unknown adapter"),
            characteristics.len()
        )];
        let mut services: Vec<Uuid> = characteristics.iter().map(|c| c.service_uuid).collect();
        services.sort();
        services.dedup();
        for service in services {
            lines.push(format!("SERVICE {service}"));
            for c in characteristics.iter().filter(|c| c.service_uuid == service) {
                lines.push(format!("  CHAR {} props={:?}", c.uuid, c.properties));
            }
        }
        Some(lines.join("\n"))
    }
}

#[async_trait]
impl Transport for BtleplugTransport {
    async fn connect(&mut self) -> Result<usize, Error> {
        if self.peripheral.is_some() || self.attempted {
            return Err(Error::InvalidState);
        }
        bounded(CONNECT_TIMEOUT, Error::ConnectTimeout, self.open()).await
    }

    async fn write(&mut self, data: &[u8]) -> Result<(), Error> {
        if self.failed {
            return Err(Error::InvalidState);
        }
        let Some(peripheral) = self.peripheral.as_mut() else {
            return Err(Error::InvalidState);
        };
        let Some(profile) = self.profile.as_ref() else {
            return Err(Error::InvalidState);
        };
        if data.len() > self.usable {
            return Err(Error::ValueTooLarge);
        }
        if self.writes == MAX_WRITES {
            return Err(Error::WriteLimit);
        }
        self.attempted = true;
        // Mark uncertain before awaiting so cancellation cannot enable replay.
        self.failed = true;
        let result = bounded(
            WRITE_TIMEOUT,
            Error::WriteTimeout,
            peripheral.write(&profile.tx, data, profile.mode.write_type()),
        )
        .await;
        result?;
        self.writes += 1;
        self.failed = false;
        Ok(())
    }

    async fn status(&mut self) -> Result<Vec<u8>, Error> {
        if self.failed || self.peripheral.is_none() {
            return Err(Error::InvalidState);
        }
        let rx_uuid = self
            .profile
            .as_ref()
            .map(|profile| profile.rx.uuid)
            .ok_or(Error::InvalidState)?;
        let peripheral = self.peripheral.as_mut().expect("checked above");
        // Only the RX characteristic's notifications are status reads; skip
        // anything else, like the BlueZ backend's per-characteristic stream.
        let notification = bounded(STATUS_TIMEOUT, Error::StatusTimeout, async {
            loop {
                match peripheral.next_notification().await {
                    Ok(Some(notification)) => {
                        if notification.uuid == rx_uuid {
                            return Ok(notification);
                        }
                    }
                    Ok(None) => return Err(Error::StatusEnded),
                    Err(error) => return Err(error),
                }
            }
        })
        .await;
        match notification {
            Ok(notification) => {
                if notification.value.len() > MAX_VALUE_SIZE {
                    return Err(Error::StatusTooLarge);
                }
                Ok(notification.value)
            }
            Err(error) => Err(error),
        }
    }

    async fn disconnect(&mut self) -> Result<(), Error> {
        self.profile = None;
        self.usable = 0;
        // Release the handle; dropping it closes our end of the link.
        let Some(mut peripheral) = self.peripheral.take() else {
            return Ok(());
        };
        bounded(
            DISCONNECT_TIMEOUT,
            Error::DisconnectTimeout,
            peripheral.disconnect(),
        )
        .await
    }

    async fn write_short_command(&mut self, data: &[u8]) -> Result<(), Error> {
        let Some(peripheral) = self.peripheral.as_mut() else {
            return Err(Error::InvalidState);
        };
        let Some(profile) = self.profile.as_ref() else {
            return Err(Error::InvalidState);
        };
        let Some(aux) = profile.aux.as_ref() else {
            return Err(Error::InvalidState);
        };
        // The short command is a fixed-size non-print write (23 bytes for
        // set-cassette-type); bound it by the transport-side write limit, not
        // by the print-chunk limit. Best effort: it does not touch the
        // attempted/failed state of the print path and is never retried.
        if data.len() > TRANSPORT_WRITE_LIMIT {
            return Err(Error::ValueTooLarge);
        }
        bounded(
            WRITE_TIMEOUT,
            Error::WriteTimeout,
            peripheral.write(aux, data, WriteType::WithResponse),
        )
        .await?;
        self.writes += 1;
        Ok(())
    }

    fn parameters(&self) -> TransportParameters {
        self.parameters.clone()
    }
}

/// Scripted GATT implementations for deterministic, hardware-independent
/// transport tests.
pub mod mock {
    use super::*;
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};

    /// Recorded state of a [`MockGattPeripheral`], shared with the test that
    /// created it so GATT operation traces can be asserted after the fact.
    #[derive(Debug, Default)]
    pub struct MockGattState {
        pub connected: bool,
        pub writes: Vec<(Uuid, Vec<u8>, WriteType)>,
        pub reads: Vec<Uuid>,
        pub subscribed: Vec<Uuid>,
        pub notifications: VecDeque<ValueNotification>,
    }

    /// Scripted peripheral. `pending_*` flags make the corresponding operation
    /// never complete (for timeout tests under paused time).
    pub struct MockGattPeripheral {
        pub characteristics: Vec<Characteristic>,
        pub state: Arc<Mutex<MockGattState>>,
        pub read_values: HashMap<Uuid, Vec<u8>>,
        pub fail_write_at: Option<usize>,
        pub pending_write: bool,
        pub pending_notifications: bool,
        pub disconnect_fails: bool,
        pub name: Option<String>,
    }

    impl MockGattPeripheral {
        pub fn new(characteristics: Vec<Characteristic>) -> (Self, Arc<Mutex<MockGattState>>) {
            let state = Arc::new(Mutex::new(MockGattState::default()));
            (
                Self {
                    characteristics,
                    state: state.clone(),
                    read_values: HashMap::new(),
                    fail_write_at: None,
                    pending_write: false,
                    pending_notifications: false,
                    disconnect_fails: false,
                    name: None,
                },
                state,
            )
        }

        /// A valid LT200B profile: vendor service with TX (write only),
        /// RX (read+notify) and AUX (write+notify).
        pub fn lt200b() -> (Self, Arc<Mutex<MockGattState>>) {
            let service = Uuid::from_u128(0xbe3dd650_2b3d_42f1_99c1_f0f749dd0678);
            let characteristic = |n: u128, properties: CharPropFlags| Characteristic {
                uuid: tail(&service, n),
                service_uuid: service,
                properties,
                descriptors: Default::default(),
            };
            Self::new(vec![
                characteristic(1, CharPropFlags::WRITE),
                characteristic(2, CharPropFlags::READ | CharPropFlags::NOTIFY),
                characteristic(3, CharPropFlags::WRITE | CharPropFlags::NOTIFY),
            ])
        }
    }

    #[async_trait]
    impl GattPeripheral for MockGattPeripheral {
        async fn connect(&mut self) -> Result<(), Error> {
            self.state.lock().unwrap().connected = true;
            Ok(())
        }

        async fn is_connected(&mut self) -> Result<bool, Error> {
            Ok(self.state.lock().unwrap().connected)
        }

        async fn disconnect(&mut self) -> Result<(), Error> {
            self.state.lock().unwrap().connected = false;
            if self.disconnect_fails {
                Err(Error::Disconnect)
            } else {
                Ok(())
            }
        }

        async fn discover_services(&mut self) -> Result<(), Error> {
            if !self.state.lock().unwrap().connected {
                return Err(Error::Profile);
            }
            Ok(())
        }

        fn characteristics(&self) -> Vec<Characteristic> {
            self.characteristics.clone()
        }

        async fn write(
            &mut self,
            characteristic: &Characteristic,
            data: &[u8],
            write_type: WriteType,
        ) -> Result<(), Error> {
            if !self.state.lock().unwrap().connected {
                return Err(Error::Write);
            }
            if self.pending_write {
                return bounded(
                    WRITE_TIMEOUT,
                    Error::WriteTimeout,
                    std::future::pending::<Result<(), Error>>(),
                )
                .await;
            }
            let mut state = self.state.lock().unwrap();
            if self.fail_write_at == Some(state.writes.len()) {
                return Err(Error::Write);
            }
            state
                .writes
                .push((characteristic.uuid, data.to_vec(), write_type));
            Ok(())
        }

        async fn read(&mut self, characteristic: &Characteristic) -> Result<Vec<u8>, Error> {
            self.state.lock().unwrap().reads.push(characteristic.uuid);
            Ok(self
                .read_values
                .get(&characteristic.uuid)
                .cloned()
                .unwrap_or_default())
        }

        async fn subscribe(&mut self, characteristic: &Characteristic) -> Result<(), Error> {
            self.state
                .lock()
                .unwrap()
                .subscribed
                .push(characteristic.uuid);
            Ok(())
        }

        async fn next_notification(&mut self) -> Result<Option<ValueNotification>, Error> {
            if self.pending_notifications {
                return bounded(
                    STATUS_TIMEOUT,
                    Error::StatusTimeout,
                    std::future::pending::<Result<Option<ValueNotification>, Error>>(),
                )
                .await;
            }
            Ok(self.state.lock().unwrap().notifications.pop_front())
        }

        async fn local_name(&mut self) -> Result<Option<String>, Error> {
            Ok(self.name.clone())
        }
    }

    /// Scripted central. The peripheral is returned by `find` after
    /// `find_polls` polls (set high to exercise the discovery timeout).
    pub struct MockGattCentral {
        pub adapter_info: String,
        pub peripheral: Option<MockGattPeripheral>,
        pub find_polls: usize,
        pub fail_scan: bool,
        pub polls: usize,
    }

    impl MockGattCentral {
        pub fn new(peripheral: MockGattPeripheral) -> Self {
            Self {
                adapter_info: "hci1 (mock)".to_string(),
                peripheral: Some(peripheral),
                find_polls: 0,
                fail_scan: false,
                polls: 0,
            }
        }
    }

    #[async_trait]
    impl GattCentral for MockGattCentral {
        async fn adapter_info(&self) -> Result<String, Error> {
            Ok(self.adapter_info.clone())
        }

        async fn find(
            &mut self,
            _address: &str,
            _timeout: Duration,
            _max_events: usize,
        ) -> Result<Box<dyn GattPeripheral>, Error> {
            if self.fail_scan {
                return Err(Error::Discovery);
            }
            self.polls += 1;
            if self.polls > self.find_polls {
                let peripheral = self.peripheral.take().ok_or(Error::DeviceNotFound)?;
                Ok(Box::new(peripheral))
            } else {
                // Never resolves: the transport-level discovery bound applies.
                return bounded(
                    DISCOVERY_TIMEOUT,
                    Error::DeviceNotFound,
                    std::future::pending::<Result<Box<dyn GattPeripheral>, Error>>(),
                )
                .await;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::mock::{MockGattCentral, MockGattPeripheral, MockGattState};
    use super::*;
    use std::sync::{Arc, Mutex};

    fn make_transport(
        peripheral: MockGattPeripheral,
    ) -> (BtleplugTransport, Arc<Mutex<MockGattState>>) {
        let state = peripheral.state.clone();
        let transport = BtleplugTransport::new("90:E5:B1:49:DA:AE")
            .unwrap()
            .with_central(Box::new(MockGattCentral::new(peripheral)))
            .with_chunk_mode(ChunkMode::Auto)
            .with_printer_cap(Some(244));
        (transport, state)
    }

    fn lt200b() -> (MockGattPeripheral, Arc<Mutex<MockGattState>>) {
        MockGattPeripheral::lt200b()
    }

    #[tokio::test]
    async fn connect_resolves_profile_and_reports_parameters() {
        let (peripheral, _state) = lt200b();
        let (mut transport, _state) = make_transport(peripheral);
        assert_eq!(transport.connect().await, Ok(244));
        let parameters = transport.parameters();
        assert_eq!(parameters.transport, Some("btleplug"));
        assert_eq!(parameters.write_mode, Some("Request"));
        assert_eq!(parameters.att_mtu, None);
        assert_eq!(parameters.gatt_value_limit, None);
        assert_eq!(parameters.printer_cap, Some(244));
        assert_eq!(parameters.effective_write, Some(244));
        assert_eq!(parameters.chunk_mode, Some("auto"));
        assert!(!parameters.experimental);
        let table = transport.describe_gatt().unwrap();
        assert!(table.contains("SERVICE"));
        assert!(table.contains("hci1 (mock)"));
    }

    #[tokio::test]
    async fn writes_are_sequential_with_response_and_exact_trace() {
        let service = Uuid::from_u128(0xbe3dd650_2b3d_42f1_99c1_f0f749dd0678);
        let tx = tail(&service, 1);
        let (peripheral, state) = lt200b();
        let (mut transport, _state) = make_transport(peripheral);
        transport.connect().await.unwrap();
        let sizes = [9usize, 242, 242, 242, 242, 242, 242, 181];
        for (i, size) in sizes.iter().enumerate() {
            transport.write(&vec![i as u8; *size]).await.unwrap();
        }
        {
            let state = state.lock().unwrap();
            // Exact GATT operation trace: same order, same bytes, every print
            // write on TX with an ATT Write Request (WithResponse).
            assert_eq!(state.writes.len(), 8);
            for (i, (uuid, data, write_type)) in state.writes.iter().enumerate() {
                assert_eq!(*uuid, tx);
                assert_eq!(*write_type, WriteType::WithResponse);
                assert_eq!(data.len(), sizes[i]);
                assert_eq!(data, &vec![i as u8; sizes[i]]);
            }
            // RX was subscribed before any write.
            assert_eq!(state.subscribed, vec![tail(&service, 2)]);
        }
        assert!(!transport.failed);
        transport.disconnect().await.unwrap();
    }

    #[tokio::test]
    async fn disconnect_failure_is_reported() {
        let (mut peripheral, _state) = lt200b();
        peripheral.disconnect_fails = true;
        let (mut transport, _state) = make_transport(peripheral);
        transport.connect().await.unwrap();
        assert_eq!(transport.disconnect().await, Err(Error::Disconnect));
    }

    #[tokio::test]
    async fn write_failure_marks_uncertain_and_never_retries() {
        let (mut peripheral, state) = lt200b();
        peripheral.fail_write_at = Some(3);
        let (mut transport, _state) = make_transport(peripheral);
        transport.connect().await.unwrap();
        transport.write(&[0; 9]).await.unwrap();
        transport.write(&[0; 242]).await.unwrap();
        transport.write(&[0; 242]).await.unwrap();
        assert_eq!(transport.write(&[0; 242]).await, Err(Error::Write));
        // No replay: the next write is rejected without touching the device.
        assert_eq!(transport.write(&[0; 242]).await, Err(Error::InvalidState));
        assert_eq!(transport.status().await, Err(Error::InvalidState));
        assert_eq!(state.lock().unwrap().writes.len(), 3);
    }

    #[tokio::test(start_paused = true)]
    async fn write_timeout_is_bounded_and_uncertain() {
        let (mut peripheral, state) = lt200b();
        peripheral.pending_write = true;
        let (mut transport, _state) = make_transport(peripheral);
        transport.connect().await.unwrap();
        assert_eq!(transport.write(&[0; 9]).await, Err(Error::WriteTimeout));
        assert_eq!(transport.write(&[0; 9]).await, Err(Error::InvalidState));
        assert_eq!(state.lock().unwrap().writes.len(), 0);
    }

    #[tokio::test]
    async fn status_reads_only_subscribed_characteristic_notifications() {
        let service = Uuid::from_u128(0xbe3dd650_2b3d_42f1_99c1_f0f749dd0678);
        let (peripheral, state) = lt200b();
        // A notification for another characteristic is skipped.
        peripheral
            .state
            .lock()
            .unwrap()
            .notifications
            .push_back(ValueNotification {
                uuid: tail(&service, 3),
                value: vec![0x1b, 0x52, 9],
            });
        peripheral
            .state
            .lock()
            .unwrap()
            .notifications
            .push_back(ValueNotification {
                uuid: tail(&service, 2),
                value: vec![0x1b, 0x52, 0],
            });
        let (mut transport, _state) = make_transport(peripheral);
        transport.connect().await.unwrap();
        assert_eq!(transport.status().await, Ok(vec![0x1b, 0x52, 0]));
        assert!(state.lock().unwrap().notifications.is_empty());
        transport.disconnect().await.unwrap();
    }

    #[tokio::test(start_paused = true)]
    async fn status_timeout_is_bounded() {
        let (mut peripheral, _state) = lt200b();
        peripheral.pending_notifications = true;
        let (mut transport, _state) = make_transport(peripheral);
        transport.connect().await.unwrap();
        assert_eq!(transport.status().await, Err(Error::StatusTimeout));
    }

    #[tokio::test(start_paused = true)]
    async fn discovery_timeout_reports_device_not_found() {
        let (peripheral, _state) = lt200b();
        let mut central = MockGattCentral::new(peripheral);
        central.find_polls = usize::MAX;
        let mut transport = BtleplugTransport::new("90:E5:B1:49:DA:AE")
            .unwrap()
            .with_central(Box::new(central));
        assert_eq!(transport.connect().await, Err(Error::ConnectTimeout));
    }

    #[tokio::test]
    async fn scan_failure_is_discovery_error() {
        let (peripheral, _state) = lt200b();
        let mut central = MockGattCentral::new(peripheral);
        central.fail_scan = true;
        let mut transport = BtleplugTransport::new("90:E5:B1:49:DA:AE")
            .unwrap()
            .with_central(Box::new(central));
        assert_eq!(transport.connect().await, Err(Error::Discovery));
    }

    #[tokio::test]
    async fn ambiguous_or_invalid_profiles_are_rejected() {
        let service = Uuid::from_u128(0xbe3dd650_2b3d_42f1_99c1_f0f749dd0678);
        let other = Uuid::from_u128(0xbe3dd650_0000_1000_8000_00805f9b34fb);
        let characteristic = |service: Uuid, n: u128, properties: CharPropFlags| Characteristic {
            uuid: tail(&service, n),
            service_uuid: service,
            properties,
            descriptors: Default::default(),
        };
        // Two vendor services: ambiguous.
        let (peripheral, _state) = MockGattPeripheral::new(vec![
            characteristic(service, 1, CharPropFlags::WRITE),
            characteristic(service, 2, CharPropFlags::NOTIFY),
            characteristic(other, 1, CharPropFlags::WRITE),
            characteristic(other, 2, CharPropFlags::NOTIFY),
        ]);
        let (mut transport, _state) = make_transport(peripheral);
        assert_eq!(transport.connect().await, Err(Error::Profile));
        // TX without any write property.
        let (peripheral, _state) = MockGattPeripheral::new(vec![
            characteristic(service, 1, CharPropFlags::READ),
            characteristic(service, 2, CharPropFlags::NOTIFY),
        ]);
        let (mut transport, _state) = make_transport(peripheral);
        assert_eq!(transport.connect().await, Err(Error::Profile));
        // RX without notify.
        let (peripheral, _state) = MockGattPeripheral::new(vec![
            characteristic(service, 1, CharPropFlags::WRITE),
            characteristic(service, 2, CharPropFlags::READ),
        ]);
        let (mut transport, _state) = make_transport(peripheral);
        assert_eq!(transport.connect().await, Err(Error::Profile));
    }

    #[tokio::test]
    async fn command_mode_is_selected_only_when_offered() {
        let service = Uuid::from_u128(0xbe3dd650_2b3d_42f1_99c1_f0f749dd0678);
        let characteristic = |n: u128, properties: CharPropFlags| Characteristic {
            uuid: tail(&service, n),
            service_uuid: service,
            properties,
            descriptors: Default::default(),
        };
        let (peripheral, _state) = MockGattPeripheral::new(vec![
            characteristic(
                1,
                CharPropFlags::WRITE | CharPropFlags::WRITE_WITHOUT_RESPONSE,
            ),
            characteristic(2, CharPropFlags::NOTIFY),
        ]);
        let (mut transport, _state) = make_transport(peripheral);
        transport.connect().await.unwrap();
        assert_eq!(transport.parameters().write_mode, Some("Command"));
    }

    #[tokio::test]
    async fn short_command_uses_aux_with_response_and_is_not_retried() {
        let service = Uuid::from_u128(0xbe3dd650_2b3d_42f1_99c1_f0f749dd0678);
        let aux = tail(&service, 3);
        let (peripheral, state) = lt200b();
        let (mut transport, _state) = make_transport(peripheral);
        transport.connect().await.unwrap();
        transport.write_short_command(&[1, 2, 3]).await.unwrap();
        {
            let state = state.lock().unwrap();
            assert_eq!(
                state.writes,
                vec![(aux, vec![1, 2, 3], WriteType::WithResponse)]
            );
        }
        // Best effort: a failed short command does not touch print state.
        transport.disconnect().await.unwrap();
    }

    #[tokio::test]
    async fn oversized_write_is_rejected_before_touching_the_device() {
        let (peripheral, state) = lt200b();
        let (mut transport, _state) = make_transport(peripheral);
        transport.connect().await.unwrap();
        assert_eq!(transport.write(&[0; 245]).await, Err(Error::ValueTooLarge));
        assert_eq!(state.lock().unwrap().writes.len(), 0);
    }
}
