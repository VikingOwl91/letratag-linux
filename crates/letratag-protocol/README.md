# letratag-protocol

Bounded, offline wire-protocol encoding for the DYMO LetraTag 200B thermal
label printer. No Bluetooth, CUPS, or hardware dependency: this crate turns a
32-row raster into the framed characteristic writes a BLE transport sends, and
parses the printer's status notifications.

[![crates.io](https://img.shields.io/crates/v/letratag-protocol)](https://crates.io/crates/letratag-protocol)
[![repository](https://img.shields.io/badge/repository-GitHub-blue)](https://github.com/VikingOwl91/letratag-linux)

Part of [letratag-linux](https://github.com/VikingOwl91/letratag-linux).

## Install

```toml
[dependencies]
letratag-protocol = "0.1.1"
```

Rust 1.98 or newer. No default features; there are no feature flags.

## Examples

### Raster model and column packing

A `Raster` is a full 32-row frame (`HEAD_ROWS`), row-major, containing only 0
(white) or 1 (black). `x` is the tape travel (feed) direction; `y = 0..31`
runs across the tape.

```rust
use letratag_protocol::{HEAD_ROWS, Raster};

let mut pixels = vec![0; 2 * HEAD_ROWS];
pixels[0] = 1; // column 0, row 0
pixels[HEAD_ROWS + 31] = 1; // column 1, row 31
let raster = Raster::new(2, pixels).unwrap();
assert!(raster.get(0, 0));
assert!(raster.get(1, 31));
assert!(!raster.get(1, 0));

// Wire packing: column x occupies bytes 4x..4x+4; row 0 is bit 7 of the
// last byte, row 31 is bit 0 of the first byte.
assert_eq!(raster.pack_columns(), vec![0, 0, 0, 0x80, 0x01, 0, 0, 0]);
assert_eq!(
    Raster::from_packed_columns(2, &raster.pack_columns()).unwrap(),
    raster
);
```

### Encoding a print job into bounded writes

`Job::writes(max_write)` returns the ordered characteristic values: a 9-byte
header first, then indexed body chunks. `max_write` is the usable
characteristic value length, NOT the raw ATT MTU. Every chunk reserves one
index byte and the final chunk two magic bytes, so the body ceiling is
`min(500, max_write - 3)`. At chunk position 27 the index 27 is skipped
(`i >= 27` sends `i + 1`), and at most 255 body chunks are produced.

```rust
use letratag_protocol::{Job, Profile, Raster};

let raster = Raster::new(4, vec![0; 4 * 32]).unwrap();
let job = Job::new(&raster, Profile::Modern).unwrap();
// Modern body: 28 bytes of framing plus 4 bytes per column.
assert_eq!(job.body().len(), 28 + 4 * 4);

let writes = job.writes(20).unwrap();
assert_eq!(writes[0].len(), 9); // header
assert!(writes.iter().all(|write| write.len() <= 20));
let last = writes.last().unwrap();
assert_eq!(&last[last.len() - 2..], &[0x12, 0x34]); // final magic
```

### Header and status parsing

```rust
use letratag_protocol::{parse_header, Job, Profile, Raster, Status};

let raster = Raster::new(1, vec![0; 32]).unwrap();
let job = Job::new(&raster, Profile::Legacy).unwrap();
let writes = job.writes(512).unwrap();
assert_eq!(parse_header(&writes[0]).unwrap(), job.body().len());

// Only code 0 is recognized as success; every other code is preserved
// unchanged as Unconfirmed.
assert_eq!(Status::parse(&[0x1b, 0x52, 0]).unwrap(), Status::Success);
assert_eq!(Status::parse(&[0x1b, 0x52, 2]).unwrap(), Status::Unconfirmed(2));
assert!(Status::parse(&[0x1b, 0x52]).is_err());
```

### Set-cassette-type short command

`set_cassette_type_write` builds one 23-byte non-print write for the
auxiliary characteristic (`HEADER + ESC s + ESC M <id> 00 00 00 + ESC Q`).
The cassette id enum is 1=6mm, 2=9mm, 3=12mm, 4=19mm, 5=24mm; the LT-200B
chassis only accepts 12 mm (`CASSETTE_ID_12MM`).

```rust
use letratag_protocol::{set_cassette_type_write, CASSETTE_ID_12MM};

let write = set_cassette_type_write(CASSETTE_ID_12MM).unwrap();
assert_eq!(write.len(), 23);
assert!(set_cassette_type_write(0).is_err());
assert!(set_cassette_type_write(6).is_err());
```

## API overview

| Item | Purpose |
|---|---|
| `Raster` | Validated 32-row binary frame: `new`, `width`, `pixels`, `get`, `pack_columns`, `from_packed_columns` |
| `Profile` | Wire profile: `Modern` (default, S2-derived) or `Legacy` (S1-derived) |
| `Job` | One encoded print job: `new`, `body`, `writes` |
| `parse_header` | Validate header framing/checksum, return the bounded body length |
| `Status` | `Success` / `Unconfirmed(u8)` status notifications: `parse` |
| `set_cassette_type_write` | Stand-alone set-cassette-type write for the auxiliary characteristic |
| `HEAD_ROWS`, `MAX_COLUMNS` | Frame height (32) and software width limit (8000 columns) |
| `CASSETTE_ID_12MM` | Cassette id 3, the only width the LT-200B chassis accepts |
| `Error` | Bounded, structured encoding/parsing errors |

## Limitations

* This crate encodes a wire format; it does not print. Transmitting writes to
  a printer is the job of a transport (see `letratag-transport`).
* The legacy profile is hardware-verified on one LT200B firmware
  (`RUTEP20220628`); the modern profile is source-backed, not
  hardware-confirmed. The implementation keeps both profiles explicit and
  never switches automatically.
* A status code 0 is not proof of a physical label: the printer reports
  success even with the lid open (phantom-success finding in the main
  repository). Completion policy belongs to the consumer.
* `MAX_COLUMNS` (8000) is a software limit, not a guaranteed device
  capability. A job that needs more than 255 chunks is rejected before any
  write.
* The literals and framing are transcribed from two MIT-licensed open-source
  references (ysfchn/dymo-bluetooth, thermal-label/letratag); see the main
  repository's `docs/protocol.md` for sources and evidence levels.

## License

MIT. See the [main repository](https://github.com/VikingOwl91/letratag-linux)
for the full license text and the reference-notice inventory in
`docs/legal/`.
