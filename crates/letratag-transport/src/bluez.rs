use super::*;
use bluer::{
    Adapter, AdapterEvent, Address, Device, DiscoveryFilter, DiscoveryTransport, Session, Uuid,
    gatt::{
        CharacteristicWriter, WriteOp,
        remote::{Characteristic, CharacteristicWriteRequest},
    },
};
use futures_util::{Stream, StreamExt};
use std::pin::Pin;

/// Bounded LE discovery wait when the adapter does not know the configured
/// device (2026-10-21 lifecycle finding). Short enough to fail fast when the
/// printer is off: it auto-powers-off after roughly five minutes idle.
const DISCOVERY_TIMEOUT: Duration = Duration::from_secs(15);
/// Bound on inspected discovery events, mirroring the discover() listing.
const MAX_DISCOVERY_EVENTS: usize = 256;

type Notifications = Pin<Box<dyn Stream<Item = Vec<u8>> + Send>>;

/// How print data is written to the TX characteristic. BlueZ flag strings:
/// "write" is the Write Request/Response operation (ATT-acknowledged),
/// "write-without-response" is the Write Command (unacknowledged).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TxMode {
    /// Write Command through an acquired writer (AcquireWrite).
    Command,
    /// Write Request through WriteValue, acknowledged by the device.
    Request,
}

/// Print-data channel selected during GATT profile resolution.
#[derive(Debug)]
enum TxChannel {
    Command(CharacteristicWriter),
    Request(Characteristic),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredDevice {
    pub address: String,
    pub name: Option<String>,
}

/// Native BlueZ transport. Construction never contacts D-Bus.
/// Dropping closes local handles; callers must explicitly await disconnect.
pub struct BluezTransport {
    address: Address,
    device: Option<Device>,
    writer: Option<TxChannel>,
    short_command: Option<Characteristic>,
    notifications: Option<Notifications>,
    /// Usable GATT value size reported by bluer (mtu_workaround applied).
    gatt_limit: usize,
    /// Effective print-chunk write limit selected at connect time:
    /// min(transport, validated printer capability, protocol bound).
    usable: usize,
    chunk_mode: ChunkMode,
    printer_cap: Option<usize>,
    parameters: TransportParameters,
    attempted: bool,
    writes: usize,
    failed: bool,
}

impl BluezTransport {
    pub fn new(address: &str) -> Result<Self, Error> {
        let address = address.parse().map_err(|_| Error::InvalidAddress)?;
        Ok(Self {
            address,
            device: None,
            writer: None,
            short_command: None,
            notifications: None,
            gatt_limit: 0,
            usable: 0,
            chunk_mode: ChunkMode::Auto,
            printer_cap: None,
            parameters: TransportParameters::unknown(),
            attempted: false,
            writes: 0,
            failed: false,
        })
    }

    /// Select how the print-chunk write size is chosen at connect time.
    /// Default: Auto.
    pub fn with_chunk_mode(mut self, mode: ChunkMode) -> Self {
        self.chunk_mode = mode;
        self
    }

    /// Record a hardware-validated printer write capability in bytes per
    /// GATT value. Only operator-confirmed limits belong here: an
    /// acknowledged ATT write does not prove the printer processed the job.
    pub fn with_printer_cap(mut self, cap: Option<usize>) -> Self {
        self.printer_cap = cap;
        self
    }

    /// Returns candidates only, including possibly cached devices. This does
    /// not establish printer identity or availability. Never powers or pairs.
    pub async fn discover(duration: Duration) -> Result<Vec<DiscoveredDevice>, Error> {
        let mut found = Vec::new();
        let scan = async {
            let session = Session::new().await.map_err(|_| Error::Discovery)?;
            let adapter = session
                .default_adapter()
                .await
                .map_err(|_| Error::Discovery)?;
            let events = adapter
                .discover_devices()
                .await
                .map_err(|_| Error::Discovery)?;
            tokio::pin!(events);
            // Bound the number of inspected events as well as retained candidates.
            for _ in 0..256 {
                let Some(event) = events.next().await else {
                    break;
                };
                let bluer::AdapterEvent::DeviceAdded(address) = event else {
                    continue;
                };
                let device = adapter.device(address).map_err(|_| Error::Discovery)?;
                let name = device.name().await.ok().flatten();
                let service_match = device
                    .uuids()
                    .await
                    .ok()
                    .flatten()
                    .is_some_and(|ids| ids.iter().any(is_service));
                if is_candidate_name(name.as_deref()) || service_match {
                    let address = address.to_string();
                    if !found
                        .iter()
                        .any(|d: &DiscoveredDevice| d.address == address)
                    {
                        found.push(DiscoveredDevice {
                            address,
                            name: name
                                .map(|n| n.chars().filter(|c| !c.is_control()).take(128).collect()),
                        });
                    }
                }
                if found.len() == 64 {
                    break;
                }
            }
            Ok::<_, Error>(())
        };
        if let Ok(result) = tokio::time::timeout(duration.min(Duration::from_secs(60)), scan).await
        {
            result?;
        }
        Ok(found)
    }

    async fn open(&mut self) -> Result<usize, Error> {
        let session = Session::new().await.map_err(|_| Error::Connect)?;
        let adapter = session
            .default_adapter()
            .await
            .map_err(|_| Error::Connect)?;
        let device = resolve_device(&adapter, self.address).await?;
        // Retain the handle before connecting so explicit cleanup also works
        // after a timeout or partially resolved profile.
        self.device = Some(device.clone());
        // Reuse an existing link instead of failing: BlueZ rejects a second
        // Connect while the device is already connected (a previous job's
        // disconnect failed, or another local client holds the link). The
        // printer is a single-central BLE peripheral, so an existing link is
        // ours; a stale link fails at the first write and stays uncertain.
        if !device.is_connected().await.map_err(|_| Error::Connect)? {
            device.connect().await.map_err(|_| Error::Connect)?;
        }
        let mut candidates = Vec::new();
        for service in device.services().await.map_err(|_| Error::Profile)? {
            let uuid = service.uuid().await.map_err(|_| Error::Profile)?;
            if is_service(&uuid) {
                candidates.push((uuid, service));
            }
        }
        if candidates.len() != 1 {
            return Err(Error::Profile);
        }
        let (service_uuid, service) = candidates.pop().ok_or(Error::Profile)?;
        let characteristics = service
            .characteristics()
            .await
            .map_err(|_| Error::Profile)?;
        let mut profile = Vec::new();
        for characteristic in &characteristics {
            let uuid = characteristic.uuid().await.map_err(|_| Error::Profile)?;
            let flags = characteristic.flags().await.map_err(|_| Error::Profile)?;
            profile.push((
                uuid,
                flags.write,
                flags.write_without_response,
                flags.notify,
            ));
        }
        let (tx, rx, mode) = profile_indices(service_uuid, &profile)?;
        // The auxiliary characteristic is the optional short-command channel
        // (set-cassette-type and similar session setup). It is not required
        // for the print path; keep the first writable match.
        self.short_command =
            short_command_index(service_uuid, &profile).map(|index| characteristics[index].clone());
        // Subscribe before acquiring a writer and before the first send.
        let notifications = characteristics[rx]
            .notify()
            .await
            .map_err(|_| Error::Connect)?;
        self.notifications = Some(Box::pin(notifications));
        // (writer, usable GATT value size, negotiated ATT MTU if known, mode)
        let (writer, gatt_limit, att_mtu, write_mode) = match mode {
            TxMode::Command => {
                let writer = characteristics[tx]
                    .write_io()
                    .await
                    .map_err(|_| Error::Connect)?;
                // bluer 0.17.4 remote::write_io applies mtu_workaround(raw_mtu),
                // subtracting five, and CharacteristicWriter::send bounds by that
                // value. Do not subtract ATT overhead again. AcquireWrite reports
                // the negotiated ATT MTU, so adding the workaround back yields it.
                let limit = writer.mtu();
                (
                    TxChannel::Command(writer),
                    limit,
                    Some(limit + 5),
                    "Command",
                )
            }
            TxMode::Request => {
                // Verified on LT200B hardware (2026-10-20): the TX characteristic
                // offers only the "write" flag, so print data goes through
                // WriteValue with an ATT Write Request. The MTU property getter
                // applies the same bluer mtu_workaround. On the LT200B BlueZ
                // reports 500 here: a characteristic capability, not the
                // negotiated link MTU, so att_mtu stays unknown rather than
                // being invented.
                let characteristic = characteristics[tx].clone();
                let limit = characteristic.mtu().await.map_err(|_| Error::Profile)?;
                (TxChannel::Request(characteristic), limit, None, "Request")
            }
        };
        if gatt_limit == 0 {
            return Err(Error::Profile);
        }
        // The effective write size is the minimum of the transport limit, the
        // validated printer capability and the protocol bound. The negotiated
        // MTU alone never proves printer support. The short-command channel
        // uses the uncapped ATT-level gatt_limit.
        let usable = effective_write_limit(gatt_limit, self.printer_cap, self.chunk_mode)?;
        self.parameters = TransportParameters {
            transport: Some("BlueZ"),
            write_mode: Some(write_mode),
            att_mtu,
            gatt_value_limit: Some(gatt_limit),
            printer_cap: self.printer_cap,
            effective_write: Some(usable),
            chunk_mode: Some(chunk_mode_label(self.chunk_mode)),
            experimental: experimental(self.chunk_mode, self.printer_cap),
        };
        self.writer = Some(writer);
        self.gatt_limit = gatt_limit;
        self.usable = usable;
        Ok(usable)
    }
}

#[async_trait]
impl Transport for BluezTransport {
    async fn connect(&mut self) -> Result<usize, Error> {
        if self.device.is_some() || self.attempted {
            return Err(Error::InvalidState);
        }
        bounded(CONNECT_TIMEOUT, Error::ConnectTimeout, self.open()).await
    }

    async fn write(&mut self, data: &[u8]) -> Result<(), Error> {
        if self.failed {
            return Err(Error::InvalidState);
        }
        if self.writer.is_none() {
            return Err(Error::InvalidState);
        }
        if data.len() > self.usable {
            return Err(Error::ValueTooLarge);
        }
        if self.writes == MAX_WRITES {
            return Err(Error::WriteLimit);
        }
        self.attempted = true;
        // Mark uncertain before awaiting so cancellation cannot enable replay.
        self.failed = true;
        let result = match self.writer.as_ref() {
            Some(TxChannel::Command(writer)) => {
                bounded(WRITE_TIMEOUT, Error::WriteTimeout, async {
                    writer.send(data).await.map_err(|_| Error::Write)
                })
                .await
            }
            Some(TxChannel::Request(characteristic)) => {
                bounded(WRITE_TIMEOUT, Error::WriteTimeout, async {
                    characteristic
                        .write_ext(data, &write_request())
                        .await
                        .map_err(|_| Error::Write)
                })
                .await
            }
            None => Err(Error::InvalidState),
        };
        result?;
        self.writes += 1;
        self.failed = false;
        Ok(())
    }

    async fn write_short_command(&mut self, data: &[u8]) -> Result<(), Error> {
        let Some(characteristic) = self.short_command.as_ref() else {
            return Err(Error::InvalidState);
        };
        // The short command is a fixed-size non-print write (23 bytes for
        // set-cassette-type); bound it by the ATT-level GATT value size, not
        // by the print-chunk limit.
        if data.len() > self.gatt_limit {
            return Err(Error::ValueTooLarge);
        }
        if self.writes == MAX_WRITES {
            return Err(Error::WriteLimit);
        }
        // Best-effort session setup, not print data: it does not touch the
        // attempted/failed state of the print path and is never retried.
        bounded(WRITE_TIMEOUT, Error::WriteTimeout, async {
            characteristic
                .write_ext(data, &write_request())
                .await
                .map_err(|_| Error::Write)
        })
        .await?;
        self.writes += 1;
        Ok(())
    }

    async fn status(&mut self) -> Result<Vec<u8>, Error> {
        if self.failed || self.writer.is_none() {
            return Err(Error::InvalidState);
        }
        let notifications = self.notifications.as_mut().ok_or(Error::InvalidState)?;
        bounded(
            STATUS_TIMEOUT,
            Error::StatusTimeout,
            next_status(notifications),
        )
        .await
    }

    async fn disconnect(&mut self) -> Result<(), Error> {
        self.writer = None;
        self.notifications = None;
        self.usable = 0;
        self.gatt_limit = 0;
        // Retain the handle until success, including when the caller cancels.
        let Some(device) = self.device.clone() else {
            return Ok(());
        };
        let result = bounded(DISCONNECT_TIMEOUT, Error::DisconnectTimeout, async {
            device.disconnect().await.map_err(|_| Error::Disconnect)
        })
        .await;
        if result.is_ok() {
            self.device = None;
        }
        result
    }

    fn parameters(&self) -> TransportParameters {
        self.parameters.clone()
    }
}

/// Diagnostics label for the configured chunk mode.
fn chunk_mode_label(mode: ChunkMode) -> &'static str {
    match mode {
        ChunkMode::Auto => "auto",
        ChunkMode::Safe => "safe",
        ChunkMode::Manual(_) => "manual",
    }
}

/// A manual size above the validated capability (or with no recorded
/// capability at all) is experimental, not validated.
fn experimental(mode: ChunkMode, printer_cap: Option<usize>) -> bool {
    match mode {
        ChunkMode::Manual(value) => printer_cap.is_none_or(|cap| value > cap),
        _ => false,
    }
}

async fn next_status(notifications: &mut Notifications) -> Result<Vec<u8>, Error> {
    let value = notifications.next().await.ok_or(Error::StatusEnded)?;
    if value.len() > MAX_VALUE_SIZE {
        return Err(Error::StatusTooLarge);
    }
    Ok(value)
}

/// Resolves the configured device on the adapter without connecting.
///
/// The BlueZ device object is not guaranteed to exist before discovery
/// (2026-10-21 lifecycle finding): an unpaired device that this BlueZ has
/// never seen, or has since removed, has no Device1 object and
/// Device1.Connect fails immediately. When the adapter does not know the
/// address, run bounded LE discovery and match the exact configured
/// address; names are discovery hints, never identity. Discovery never
/// pairs, bonds or trusts the device, and dropping the event stream stops
/// the session (bluer single-session token).
pub async fn resolve_device(adapter: &Adapter, address: Address) -> Result<Device, Error> {
    if adapter
        .device_addresses()
        .await
        .map_err(|_| Error::Connect)?
        .contains(&address)
    {
        return adapter.device(address).map_err(|_| Error::Connect);
    }
    // Best effort: an LE-only filter avoids BR/EDR inquiry noise. Another
    // client's active session may merge or reject the filter; matching by
    // exact address stays correct either way.
    let _ = adapter
        .set_discovery_filter(DiscoveryFilter {
            transport: DiscoveryTransport::Le,
            ..Default::default()
        })
        .await;
    let mut events = adapter
        .discover_devices()
        .await
        .map_err(|_| Error::Discovery)?;
    await_discovered(&mut events, address).await?;
    adapter.device(address).map_err(|_| Error::Connect)
}

/// Waits until the exact target address appears in a discovery event
/// stream. Bounded by DISCOVERY_TIMEOUT and MAX_DISCOVERY_EVENTS; only
/// DeviceAdded events match. The stream may first replay already-known
/// devices, so a device that appeared between the known-check and the
/// discovery start still resolves immediately.
async fn await_discovered<S>(events: &mut S, target: Address) -> Result<(), Error>
where
    S: Stream<Item = AdapterEvent> + Unpin,
{
    let scan = async {
        for _ in 0..MAX_DISCOVERY_EVENTS {
            match events.next().await {
                Some(AdapterEvent::DeviceAdded(address)) if address == target => return Ok(()),
                Some(_) => continue,
                None => break,
            }
        }
        Err(Error::DeviceNotFound)
    };
    match tokio::time::timeout(DISCOVERY_TIMEOUT, scan).await {
        Ok(result) => result,
        Err(_) => Err(Error::DeviceNotFound),
    }
}

fn is_service(uuid: &Uuid) -> bool {
    uuid.as_u128() >> 96 == 0xbe3d_d650
}

fn is_candidate_name(name: Option<&str>) -> bool {
    name.is_some_and(|name| {
        let name = name.to_ascii_lowercase();
        name.starts_with("letratag") || name.starts_with("dymo lt-200b")
    })
}

/// Selects the write mode for a TX characteristic from its BlueZ flags.
/// `write` is the Write Request flag, `write_without_response` the Write
/// Command flag. The low-overhead Write Command is preferred when the device
/// offers both. A TX characteristic without any write flag is rejected.
fn tx_mode(write: bool, write_without_response: bool) -> Option<TxMode> {
    match (write, write_without_response) {
        (_, true) => Some(TxMode::Command),
        (true, false) => Some(TxMode::Request),
        (false, false) => None,
    }
}

/// WriteValue request with an ATT Write Request (with response), required for
/// characteristics that only offer the "write" flag. bluer's WriteOp default is
/// the unacknowledged Command, so the type must be set explicitly.
fn write_request() -> CharacteristicWriteRequest {
    CharacteristicWriteRequest {
        op_type: WriteOp::Request,
        ..Default::default()
    }
}

/// Index of the auxiliary (short-command) characteristic: the service tail
/// plus three, offering at least one write mode. Optional: `None` when the
/// device has no writable auxiliary characteristic.
fn short_command_index(
    service: Uuid,
    characteristics: &[(Uuid, bool, bool, bool)],
) -> Option<usize> {
    let aux = Uuid::from_u128(service.as_u128() + (3u128 << 96));
    characteristics
        .iter()
        .position(|(uuid, write, without_response, _)| {
            *uuid == aux && (*write || *without_response)
        })
}

/// Characteristics are (UUID, "write", "write-without-response", "notify").
fn profile_indices(
    service: Uuid,
    characteristics: &[(Uuid, bool, bool, bool)],
) -> Result<(usize, usize, TxMode), Error> {
    if !is_service(&service) {
        return Err(Error::Profile);
    }
    let tx_uuid = Uuid::from_u128(service.as_u128() + (1u128 << 96));
    let rx_uuid = Uuid::from_u128(service.as_u128() + (2u128 << 96));
    let tx: Vec<_> = characteristics
        .iter()
        .enumerate()
        .filter(|(_, (uuid, _, _, _))| *uuid == tx_uuid)
        .collect();
    let rx: Vec<_> = characteristics
        .iter()
        .enumerate()
        .filter(|(_, (uuid, _, _, _))| *uuid == rx_uuid)
        .collect();
    match (&tx[..], &rx[..]) {
        ([(tx, (_, write, without_response, _))], [(rx, (_, _, _, true))]) => {
            let mode = tx_mode(*write, *without_response).ok_or(Error::Profile)?;
            Ok((*tx, *rx, mode))
        }
        _ => Err(Error::Profile),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures_util::stream;

    #[test]
    fn construction_only_parses_address() {
        assert!(BluezTransport::new("00:11:22:33:44:55").is_ok());
        assert!(matches!(
            BluezTransport::new("invalid"),
            Err(Error::InvalidAddress)
        ));
    }

    #[test]
    fn construction_defaults_to_auto_without_a_validated_cap() {
        let transport = BluezTransport::new("00:11:22:33:44:55").unwrap();
        assert_eq!(transport.chunk_mode, ChunkMode::Auto);
        assert_eq!(transport.printer_cap, None);
        assert_eq!(transport.parameters(), TransportParameters::unknown());
        let configured = transport
            .with_chunk_mode(ChunkMode::Manual(244))
            .with_printer_cap(Some(244));
        assert_eq!(configured.chunk_mode, ChunkMode::Manual(244));
        assert_eq!(configured.printer_cap, Some(244));
    }

    #[test]
    fn chunk_mode_labels_match_the_cli_names() {
        assert_eq!(chunk_mode_label(ChunkMode::Auto), "auto");
        assert_eq!(chunk_mode_label(ChunkMode::Safe), "safe");
        assert_eq!(chunk_mode_label(ChunkMode::Manual(20)), "manual");
    }

    #[test]
    fn only_manual_sizes_above_the_validated_cap_are_experimental() {
        assert!(!experimental(ChunkMode::Auto, Some(244)));
        assert!(!experimental(ChunkMode::Safe, None));
        assert!(!experimental(ChunkMode::Manual(244), Some(244)));
        assert!(!experimental(ChunkMode::Manual(128), Some(244)));
        assert!(experimental(ChunkMode::Manual(245), Some(244)));
        // No recorded capability: any manual size is unvalidated.
        assert!(experimental(ChunkMode::Manual(20), None));
        assert!(experimental(ChunkMode::Manual(244), None));
    }

    #[test]
    fn profile_requires_same_tail_and_exact_flags_and_unambiguous_ids() {
        let service = Uuid::from_u128(0xbe3dd650_0000_1000_8000_00805f9b34fb);
        let tx = Uuid::from_u128(service.as_u128() + (1u128 << 96));
        let rx = Uuid::from_u128(service.as_u128() + (2u128 << 96));
        // (uuid, "write", "write-without-response", "notify"); a TX with the
        // Write Command flag selects the AcquireWrite channel.
        assert_eq!(
            profile_indices(
                service,
                &[(rx, false, false, true), (tx, false, true, false)]
            ),
            Ok((1, 0, TxMode::Command))
        );
        // A TX characteristic without any write flag is rejected.
        assert_eq!(
            profile_indices(
                service,
                &[(tx, false, false, false), (rx, false, false, true)]
            ),
            Err(Error::Profile)
        );
        assert_eq!(
            profile_indices(
                service,
                &[
                    (tx, false, true, false),
                    (rx, false, false, true),
                    (tx, false, true, false)
                ]
            ),
            Err(Error::Profile)
        );
        assert_eq!(
            profile_indices(
                service,
                &[
                    (Uuid::from_u128(tx.as_u128() + 1), false, true, false),
                    (rx, false, false, true)
                ]
            ),
            Err(Error::Profile)
        );
        // An RX characteristic without notify is rejected.
        assert_eq!(
            profile_indices(
                service,
                &[(tx, false, true, false), (rx, false, false, false)]
            ),
            Err(Error::Profile)
        );
    }

    #[test]
    fn verified_lt200b_profile_selects_write_request() {
        // Observed on the physical LT200B on 2026-10-20 (see
        // docs/evidence/gatt-inventory-2026-10-20.md): TX be3dd651 offers only
        // the "write" flag (Write Request), RX be3dd652 offers read+notify,
        // and auxiliary be3dd653 offers read+write+notify.
        let service = Uuid::from_u128(0xbe3dd650_2b3d_42f1_99c1_f0f749dd0678);
        let tx = Uuid::from_u128(service.as_u128() + (1u128 << 96));
        let rx = Uuid::from_u128(service.as_u128() + (2u128 << 96));
        let aux = Uuid::from_u128(service.as_u128() + (3u128 << 96));
        assert_eq!(
            profile_indices(
                service,
                &[
                    (tx, true, false, false),
                    (rx, false, false, true),
                    (aux, true, false, true)
                ]
            ),
            Ok((0, 1, TxMode::Request))
        );
    }

    #[test]
    fn short_command_index_requires_the_aux_tail_and_a_write_flag() {
        let service = Uuid::from_u128(0xbe3dd650_0000_1000_8000_00805f9b34fb);
        let tx = Uuid::from_u128(service.as_u128() + (1u128 << 96));
        let rx = Uuid::from_u128(service.as_u128() + (2u128 << 96));
        let aux = Uuid::from_u128(service.as_u128() + (3u128 << 96));
        // Verified LT200B profile: aux offers write.
        assert_eq!(
            short_command_index(
                service,
                &[
                    (tx, true, false, false),
                    (rx, false, false, true),
                    (aux, true, false, true)
                ]
            ),
            Some(2)
        );
        // No auxiliary characteristic.
        assert_eq!(
            short_command_index(
                service,
                &[(tx, true, false, false), (rx, false, false, true)]
            ),
            None
        );
        // Auxiliary without any write flag.
        assert_eq!(
            short_command_index(
                service,
                &[
                    (tx, true, false, false),
                    (rx, false, false, true),
                    (aux, false, false, true)
                ]
            ),
            None
        );
    }

    #[test]
    fn tx_mode_prefers_command_and_requires_a_write_flag() {
        assert_eq!(tx_mode(false, false), None);
        assert_eq!(tx_mode(true, false), Some(TxMode::Request));
        assert_eq!(tx_mode(false, true), Some(TxMode::Command));
        assert_eq!(tx_mode(true, true), Some(TxMode::Command));
    }

    #[test]
    fn write_request_uses_an_acknowledged_write_request() {
        let request = write_request();
        assert_eq!(request.op_type, WriteOp::Request);
        assert_eq!(request.offset, 0);
    }

    #[test]
    fn candidate_names_are_only_discovery_hints() {
        assert!(is_candidate_name(Some("Letratag 200B")));
        assert!(is_candidate_name(Some("DYMO LT-200B")));
        assert!(!is_candidate_name(Some("Other printer")));
        assert!(!is_candidate_name(None));
    }

    #[tokio::test]
    async fn ended_and_oversized_notifications_are_errors() {
        let mut ended: Notifications = Box::pin(stream::empty());
        assert_eq!(next_status(&mut ended).await, Err(Error::StatusEnded));
        let mut oversized: Notifications = Box::pin(stream::iter([vec![0; 513]]));
        assert_eq!(
            next_status(&mut oversized).await,
            Err(Error::StatusTooLarge)
        );
        let mut status: Notifications = Box::pin(stream::iter([vec![0x1b, 0x52, 1]]));
        assert_eq!(next_status(&mut status).await, Ok(vec![0x1b, 0x52, 1]));
    }

    #[tokio::test(start_paused = true)]
    async fn pending_operations_obey_their_deadlines() {
        for (duration, error) in [
            (CONNECT_TIMEOUT, Error::ConnectTimeout),
            (WRITE_TIMEOUT, Error::WriteTimeout),
            (STATUS_TIMEOUT, Error::StatusTimeout),
            (DISCONNECT_TIMEOUT, Error::DisconnectTimeout),
        ] {
            let start = tokio::time::Instant::now();
            assert_eq!(
                bounded::<()>(duration, error, std::future::pending()).await,
                Err(error)
            );
            assert_eq!(start.elapsed(), duration);
        }
    }

    fn addr(text: &str) -> Address {
        text.parse().unwrap()
    }

    #[test]
    fn device_not_found_is_actionable_and_distinct() {
        // A connection-establishment failure: the engine maps it to
        // SafeTransport (nothing was sent), never to an uncertain outcome.
        assert!(Error::DeviceNotFound.to_string().contains("powered on"));
        assert_ne!(Error::DeviceNotFound, Error::Connect);
        assert_ne!(Error::DeviceNotFound, Error::Discovery);
    }

    #[tokio::test]
    async fn discovery_wait_matches_only_the_exact_address() {
        let target = addr("90:E5:B1:49:DA:AE");
        let events = vec![
            AdapterEvent::DeviceAdded(addr("10:D9:A2:4E:6C:3F")),
            AdapterEvent::DeviceRemoved(target),
            AdapterEvent::DeviceAdded(target),
        ];
        let mut stream = stream::iter(events);
        assert_eq!(await_discovered(&mut stream, target).await, Ok(()));
    }

    #[tokio::test]
    async fn discovery_wait_ends_when_the_stream_ends_without_the_target() {
        let target = addr("90:E5:B1:49:DA:AE");
        let mut stream = stream::iter([AdapterEvent::DeviceAdded(addr("10:D9:A2:4E:6C:3F"))]);
        assert_eq!(
            await_discovered(&mut stream, target).await,
            Err(Error::DeviceNotFound)
        );
    }

    #[tokio::test]
    async fn discovery_wait_stops_after_the_event_cap() {
        let target = addr("90:E5:B1:49:DA:AE");
        let noise = || AdapterEvent::DeviceAdded(addr("10:D9:A2:4E:6C:3F"));
        let mut stream = stream::iter(std::iter::repeat_with(noise).take(MAX_DISCOVERY_EVENTS));
        assert_eq!(
            await_discovered(&mut stream, target).await,
            Err(Error::DeviceNotFound)
        );
    }

    #[tokio::test(start_paused = true)]
    async fn discovery_wait_is_bounded_by_its_timeout() {
        let target = addr("90:E5:B1:49:DA:AE");
        let mut stream = stream::pending();
        let start = tokio::time::Instant::now();
        assert_eq!(
            await_discovered(&mut stream, target).await,
            Err(Error::DeviceNotFound)
        );
        assert_eq!(start.elapsed(), DISCOVERY_TIMEOUT);
    }
}
