# letratag-transport

Bounded transports for DYMO LetraTag printers: an in-process mock transport
for tests and CI, and two real-hardware transports behind feature flags, the
BlueZ transport (`ble`, over D-Bus) and the btleplug transport
(`btleplug`). All of them share one asynchronous interface with bounded
timeouts, adaptive GATT chunk sizing, and conservative semantics: a
successful transport job is never evidence of physical printing.

[![crates.io](https://img.shields.io/crates/v/letratag-transport)](https://crates.io/crates/letratag-transport)
[![repository](https://img.shields.io/badge/repository-GitHub-blue)](https://github.com/VikingOwl91/letratag-linux)

Part of [letratag-linux](https://github.com/VikingOwl91/letratag-linux).

## Install

Mock transport only (safe default, no Bluetooth):

```toml
[dependencies]
letratag-transport = "0.1.2"
```

Real hardware over BlueZ:

```toml
[dependencies]
letratag-transport = { version = "0.1.2", features = ["ble"] }
```

Real hardware over btleplug:

```toml
[dependencies]
letratag-transport = { version = "0.1.2", features = ["btleplug"] }
```

The `ble` feature needs the D-Bus development library at build time and access
to the system BlueZ D-Bus API at runtime (the same permission `bluetoothctl`
uses). No pairing is required or performed. The `btleplug` feature needs the
btleplug stack and the platform's native Bluetooth backend.

## Examples

### Selecting the effective write size

`effective_write_limit` is the minimum of the negotiated transport GATT value
limit, a hardware-validated printer capability, and the protocol
implementation bound. The negotiated MTU alone never proves printer support.

```rust
use letratag_transport::{
    effective_write_limit, ChunkMode, CONSERVATIVE_WRITE_LIMIT, Error, PROTOCOL_WRITE_LIMIT,
};

// auto: the validated capability wins when it is the smallest bound;
// without a recorded capability the conservative ATT23-safe limit applies.
assert_eq!(effective_write_limit(495, Some(244), ChunkMode::Auto), Ok(244));
assert_eq!(
    effective_write_limit(495, None, ChunkMode::Auto),
    Ok(CONSERVATIVE_WRITE_LIMIT)
);
// safe: always the conservative limit.
assert_eq!(
    effective_write_limit(495, Some(244), ChunkMode::Safe),
    Ok(CONSERVATIVE_WRITE_LIMIT)
);
// manual: validated against the negotiated transport limit and the
// protocol bounds 9..=512.
assert_eq!(
    effective_write_limit(495, Some(244), ChunkMode::Manual(128)),
    Ok(128)
);
assert_eq!(
    effective_write_limit(100, None, ChunkMode::Manual(128)),
    Err(Error::WriteSizeExceedsTransport {
        configured: 128,
        limit: 100
    })
);
assert_eq!(
    effective_write_limit(495, None, ChunkMode::Manual(600)),
    Err(Error::InvalidWriteSize(600))
);
// The protocol implementation bound caps everything.
assert_eq!(
    effective_write_limit(600, Some(600), ChunkMode::Auto),
    Ok(PROTOCOL_WRITE_LIMIT)
);
```

### Inspecting transport parameters

`Transport::parameters` reports the connection-time parameters for
diagnostics. Unknown values are reported explicitly, never invented.

```rust
use letratag_transport::TransportParameters;

let parameters = TransportParameters {
    transport: Some("BlueZ"),
    write_mode: Some("Request"),
    att_mtu: None,
    gatt_value_limit: Some(495),
    printer_cap: Some(244),
    effective_write: Some(244),
    chunk_mode: Some("auto"),
    experimental: false,
};
let text = parameters.describe();
assert!(text.contains("Transport:"));
assert!(text.contains("Effective write size:"));
assert!(text.contains("244"));
```

### A complete mock job

The mock transport records exact writes and returns raw status notifications,
without interpreting printer completion.

```rust
use letratag_transport::{MockTransport, Transport};

#[tokio::main]
async fn main() {
    let mut transport = MockTransport::default();
    // connect() returns the usable characteristic value size.
    assert_eq!(transport.connect().await, Ok(512));
    transport.write(b"header").await.unwrap();
    transport.write(&[0x00, 0xff]).await.unwrap();
    assert_eq!(transport.writes(), &[b"header".to_vec(), vec![0x00, 0xff]]);
    assert_eq!(transport.status().await, Ok(vec![0x1b, 0x52, 0]));
    transport.disconnect().await.unwrap();
}
```

### Fault injection: no blind retries

A failed write leaves delivery uncertain; the transport never retries on its
own.

```rust
use letratag_transport::{Error, MockFault, MockTransport, Transport};

#[tokio::main]
async fn main() {
    let mut transport = MockTransport::with_fault(MockFault::WriteAt(0));
    transport.connect().await.unwrap();
    assert_eq!(transport.write(b"payload").await, Err(Error::Write));
    assert_eq!(transport.write(b"payload").await, Err(Error::InvalidState));
    transport.disconnect().await.unwrap();
}
```

### Constructing the BlueZ transport (feature `ble`)

Construction never contacts D-Bus. Print jobs need an explicit `connect()`,
which resolves the configured device (running bounded LE discovery when the
adapter does not know it) with bounded timeouts.

```rust
# #[cfg(feature = "ble")]
# fn main() {
use letratag_transport::{BluezTransport, ChunkMode, Transport};

let transport = BluezTransport::new("90:E5:B1:49:DA:AE")
    .unwrap()
    .with_chunk_mode(ChunkMode::Auto)
    .with_printer_cap(Some(244));
// Before connect, every parameter is reported unknown.
assert_eq!(transport.parameters().effective_write, None);
# }
# #[cfg(not(feature = "ble"))]
# fn main() {}
```

### Constructing the btleplug transport (feature `btleplug`)

The same interface and the same semantics over the btleplug stack. BlueZ
remains the default real-hardware backend of the application; both were
compared on real hardware and produced identical payloads, GATT operations
and write mode.

```rust
# #[cfg(feature = "btleplug")]
# fn main() {
use letratag_transport::{BtleplugTransport, ChunkMode, Transport};

let transport = BtleplugTransport::new("90:E5:B1:49:DA:AE")
    .unwrap()
    .with_chunk_mode(ChunkMode::Auto)
    .with_printer_cap(Some(244));
assert_eq!(transport.parameters().effective_write, None);
# }
# #[cfg(not(feature = "btleplug"))]
# fn main() {}
```

## API overview

| Item | Purpose |
|---|---|
| `Transport` | Async interface: `connect`, `write`, `status`, `disconnect`, `write_short_command` (default: unsupported), `parameters` |
| `MockTransport` | In-process test double: `default`, `with_fault`, `writes`, `short_commands`, `status_reads` |
| `MockFault` | Deterministic faults: `ConnectTimeout`, `WriteAt`, `WriteBlock`, `StatusTimeout`, `StatusCode`, `StatusCodes`, `Disconnect` |
| `ChunkMode` | Write-size policy: `Auto`, `Safe`, `Manual(usize)` |
| `effective_write_limit` | `min(transport, validated printer cap, protocol bound)` selection and validation |
| `TransportParameters` | Connection-time parameters for diagnostics: `unknown`, `describe` |
| `CONSERVATIVE_WRITE_LIMIT`, `PROTOCOL_WRITE_LIMIT` | 20-byte ATT23-safe fallback; 503-byte protocol implementation bound |
| `Error` | Bounded, structured transport errors (timeouts, profile, write, discovery, write-size validation) |
| `BluezTransport` (feature `ble`) | Real hardware over BlueZ/D-Bus: `new`, `with_chunk_mode`, `with_printer_cap`, `discover` |
| `BtleplugTransport` (feature `btleplug`) | Real hardware over btleplug: `new`, `with_chunk_mode`, `with_printer_cap` |
| `DiscoveredDevice`, `resolve_device` (feature `ble`) | Discovery candidates; device resolution with bounded LE discovery for absent devices |

## Semantics and limitations

* Bounded timeouts on every operation: connect 10 s, write 3 s, status 30 s,
  disconnect 3 s. Discovery for an absent device is bounded to 15 s and 256
  inspected events, matching the exact configured address.
* No uncontrolled retries. After a write begins, an error, timeout, or
  cancellation leaves the delivery uncertain; the caller decides. The mock
  transport models this with `MockFault::WriteBlock` and a permanently failed
  state.
* A successful transport job is NOT evidence of physical printing. The
  LetraTag 200B reports status code 0 even with the lid open (phantom
  success), so completion and journal policy belong to the consumer (see the
  `letratag-printer` engine in the main repository).
* The BlueZ transport never pairs, bonds, trusts, or changes persistent
  Bluetooth configuration. It prefers an acknowledged ATT Write Request or
  Write Command depending on the characteristic flags, validates the GATT
  profile (exactly one vendor service, unique TX/RX with matching tails),
  and subscribes to notifications before the first write.
* Chunk sizing never assumes the negotiated MTU proves printer support, and
  never silently resizes after a partially transmitted job. Manual sizes
  above the validated capability are reported as experimental. An
  acknowledged ATT write is not validation.
* The mock transport is a test double: its completion proves nothing about
  hardware.

## License

MIT. See the [main repository](https://github.com/VikingOwl91/letratag-linux)
for the full license text and the reference-notice inventory in
`docs/legal/`.
