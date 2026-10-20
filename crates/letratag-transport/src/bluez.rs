use super::*;
use bluer::{
    Address, Device, Session, Uuid,
    gatt::{
        CharacteristicWriter, WriteOp,
        remote::{Characteristic, CharacteristicWriteRequest},
    },
};
use futures_util::{Stream, StreamExt};
use std::pin::Pin;

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
    /// ATT-level usable value size (bluer mtu_workaround applied), without
    /// the print-chunk calibration cap.
    mtu: usize,
    usable: usize,
    /// Print-chunk value size cap. Default 20 (ATT23-safe) until the
    /// printer's larger-value support is calibrated on hardware.
    max_write: Option<usize>,
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
            mtu: 0,
            usable: 0,
            max_write: None,
            attempted: false,
            writes: 0,
            failed: false,
        })
    }

    /// Override the print-chunk value size cap (calibration only).
    pub fn with_max_write(mut self, max_write: Option<usize>) -> Self {
        self.max_write = max_write;
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
        let device = adapter.device(self.address).map_err(|_| Error::Connect)?;
        // Retain the handle before connecting so explicit cleanup also works
        // after a timeout or partially resolved profile.
        self.device = Some(device.clone());
        device.connect().await.map_err(|_| Error::Connect)?;
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
        // Print chunks are capped at ATT23-safe 20 until the printer's
        // larger-value support is calibrated on hardware; the short-command
        // channel uses the uncapped ATT-level mtu.
        let cap = self.max_write.unwrap_or(20);
        let (writer, mtu, usable) = match mode {
            TxMode::Command => {
                let writer = characteristics[tx]
                    .write_io()
                    .await
                    .map_err(|_| Error::Connect)?;
                // bluer 0.17.4 remote::write_io applies mtu_workaround(raw_mtu),
                // subtracting five, and CharacteristicWriter::send bounds by that
                // value. Do not subtract ATT overhead again.
                let mtu = writer.mtu();
                (TxChannel::Command(writer), mtu, mtu.min(cap))
            }
            TxMode::Request => {
                // Verified on LT200B hardware (2026-10-20): the TX characteristic
                // offers only the "write" flag, so print data goes through
                // WriteValue with an ATT Write Request. The MTU property getter
                // applies the same bluer mtu_workaround.
                let characteristic = characteristics[tx].clone();
                let mtu = characteristic.mtu().await.map_err(|_| Error::Profile)?;
                (TxChannel::Request(characteristic), mtu, mtu.min(cap))
            }
        };
        if usable == 0 {
            return Err(Error::Profile);
        }
        self.writer = Some(writer);
        self.mtu = mtu;
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
        // set-cassette-type); bound it by the ATT-level MTU, not by the
        // print-chunk calibration cap.
        if data.len() > self.mtu {
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
}

async fn next_status(notifications: &mut Notifications) -> Result<Vec<u8>, Error> {
    let value = notifications.next().await.ok_or(Error::StatusEnded)?;
    if value.len() > MAX_VALUE_SIZE {
        return Err(Error::StatusTooLarge);
    }
    Ok(value)
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
}
