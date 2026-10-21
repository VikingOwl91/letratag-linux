//! Read-only hardware information extraction over BlueZ. Diagnostic only:
//! dumps the full GATT table, reads the status characteristics and watches
//! RX/AUX notifications. Never sends a print job.

use bluer::{
    Address, Session, Uuid,
    gatt::{WriteOp, remote::CharacteristicWriteRequest},
};
use futures_util::StreamExt;
use std::time::{Duration, Instant};

const GAP_2AA6: &str = "00002aa6-0000-1000-8000-00805f9b34fb";

fn tail(service: &Uuid, n: u128) -> Uuid {
    Uuid::from_u128(service.as_u128() + (n << 96))
}

fn hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<Vec<_>>()
        .join("")
}

fn ascii(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|b| {
            if (0x20..0x7f).contains(b) {
                char::from(*b)
            } else {
                '.'
            }
        })
        .collect()
}

pub async fn run(
    address: &str,
    watch: Duration,
    set_cassette_type: Option<u8>,
) -> Result<(), Box<dyn std::error::Error>> {
    let address: Address = address.parse().map_err(|_| "invalid device address")?;
    let started = Instant::now();
    let session = Session::new()
        .await
        .map_err(|e| format!("BlueZ session: {e}"))?;
    let adapter = session
        .default_adapter()
        .await
        .map_err(|e| format!("default adapter: {e}"))?;
    // Shared lifecycle: resolves the configured device, running bounded LE
    // discovery when the adapter does not know it (2026-10-21 finding).
    let device = letratag_transport::resolve_device(&adapter, address)
        .await
        .map_err(|e| format!("resolve device: {e}"))?;
    device
        .connect()
        .await
        .map_err(|e| format!("connect: {e}"))?;
    println!("connected to {address} after {:?}", started.elapsed());

    let services = device
        .services()
        .await
        .map_err(|e| format!("services: {e}"))?;
    let mut all_chars: Vec<(Uuid, bluer::gatt::remote::Characteristic)> = Vec::new();
    let mut vendor = None;
    for service in &services {
        let uuid = service
            .uuid()
            .await
            .map_err(|e| format!("service uuid: {e}"))?;
        if uuid.as_u128() >> 96 == 0xbe3d_d650 {
            vendor = Some(uuid);
        }
        println!("SERVICE {uuid} handle={:#06x}", service.id());
        for characteristic in service
            .characteristics()
            .await
            .map_err(|e| format!("characteristics: {e}"))?
        {
            let cuuid = characteristic
                .uuid()
                .await
                .map_err(|e| format!("char uuid: {e}"))?;
            let flags = characteristic
                .flags()
                .await
                .map_err(|e| format!("flags: {e}"))?;
            let mut props = Vec::new();
            if flags.read {
                props.push("read");
            }
            if flags.write {
                props.push("write");
            }
            if flags.write_without_response {
                props.push("write-without-response");
            }
            if flags.notify {
                props.push("notify");
            }
            if flags.indicate {
                props.push("indicate");
            }
            let value = if flags.read {
                match characteristic.read().await {
                    Ok(v) => format!("{} ({:?})", hex(&v), ascii(&v)),
                    Err(e) => format!("read error: {e}"),
                }
            } else {
                "not readable".to_string()
            };
            println!(
                "  CHAR {cuuid} handle={:#06x} [{}] {value}",
                characteristic.id(),
                props.join(",")
            );
            for descriptor in characteristic
                .descriptors()
                .await
                .map_err(|e| format!("descriptors: {e}"))?
            {
                let duuid = descriptor
                    .uuid()
                    .await
                    .map_err(|e| format!("desc uuid: {e}"))?;
                let value = match descriptor.read().await {
                    Ok(v) => hex(&v),
                    Err(e) => format!("read error: {e}"),
                };
                println!("    DESC {duuid} handle={:#06x} {value}", descriptor.id());
            }
            all_chars.push((cuuid, characteristic));
        }
    }
    let by_uuid = |needle: &Uuid| {
        all_chars
            .iter()
            .find(|(uuid, _)| uuid == needle)
            .map(|(_, c)| c.clone())
    };
    let Some(vendor) = vendor else {
        device.disconnect().await.ok();
        return Err("vendor service be3dd650 not found".into());
    };
    let (Some(rx), Some(aux)) = (by_uuid(&tail(&vendor, 2)), by_uuid(&tail(&vendor, 3))) else {
        device.disconnect().await.ok();
        return Err("RX/AUX characteristics not found".into());
    };
    let gap_2aa6 = GAP_2AA6.parse::<Uuid>().ok().and_then(|u| by_uuid(&u));

    let mut rx_stream = Box::pin(rx.notify().await.map_err(|e| format!("RX notify: {e}"))?);
    let mut aux_stream = Box::pin(aux.notify().await.map_err(|e| format!("AUX notify: {e}"))?);
    println!("subscribed to RX + AUX notifications");

    dump_state("idle", &rx, &aux, gap_2aa6.as_ref()).await;

    if let Some(id) = set_cassette_type {
        let command = letratag_protocol::set_cassette_type_write(id)
            .map_err(|e| format!("cassette id: {e}"))?;
        aux.write_ext(
            &command,
            &CharacteristicWriteRequest {
                op_type: WriteOp::Request,
                ..Default::default()
            },
        )
        .await
        .map_err(|e| format!("short command write: {e}"))?;
        println!(
            "sent ESC M short command ({} bytes): {}",
            command.len(),
            hex(&command)
        );
        tokio::time::sleep(Duration::from_secs(3)).await;
        dump_state("after-escm", &rx, &aux, gap_2aa6.as_ref()).await;
    }

    println!("watching notifications for {watch:?}...");
    let deadline = Instant::now() + watch;
    let mut count = 0usize;
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            break;
        }
        let rx_next = rx_stream.next();
        let aux_next = aux_stream.next();
        tokio::select! {
            r = tokio::time::timeout(remaining, rx_next) => match r {
                Ok(Some(v)) => { count += 1; println!("NOTIFY RX: {}", hex(&v)); }
                Ok(None) => break,
                Err(_) => break,
            },
            a = tokio::time::timeout(remaining, aux_next) => match a {
                Ok(Some(v)) => { count += 1; println!("NOTIFY AUX: {}", hex(&v)); }
                Ok(None) => break,
                Err(_) => break,
            },
        }
    }
    println!(
        "watch ended after {:?} with {count} notification(s)",
        started.elapsed()
    );
    dump_state("final", &rx, &aux, gap_2aa6.as_ref()).await;
    device
        .disconnect()
        .await
        .map_err(|e| format!("disconnect: {e}"))?;
    println!("disconnected");
    Ok(())
}

async fn dump_state(
    label: &str,
    rx: &bluer::gatt::remote::Characteristic,
    aux: &bluer::gatt::remote::Characteristic,
    gap_2aa6: Option<&bluer::gatt::remote::Characteristic>,
) {
    let aux_v = aux
        .read()
        .await
        .map(|v| hex(&v))
        .unwrap_or_else(|e| format!("err {e}"));
    let rx_v = rx
        .read()
        .await
        .map(|v| hex(&v))
        .unwrap_or_else(|e| format!("err {e}"));
    let gap_v = match gap_2aa6 {
        Some(c) => c
            .read()
            .await
            .map(|v| hex(&v))
            .unwrap_or_else(|e| format!("err {e}")),
        None => "n/a".to_string(),
    };
    println!("STATE {label}: aux={aux_v} rx={rx_v} gap_2aa6={gap_v}");
}
