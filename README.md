# letratag-linux

Standalone Rust driver for the DYMO LetraTag 200B on Linux.
PAPPL provides a local IPP print destination; BlueZ is accessed over D-Bus.
No Python runtime, DYMO app, HomeOps, or Snipe-IT dependency.

**Development status: first physical print achieved and operator-confirmed
(2026-10-20, see [verification.md](docs/verification.md) and
[hardware-acceptance.md](docs/hardware-acceptance.md)).** The IPP service is
deliberately an explicitly named **mock destination**; the BLE transport is
experimental and calibration is ongoing. This is not a production printer
driver. See [verification.md](docs/verification.md) for the actual test
status.

## Architecture

```
Linux application / lp / IPP
          ↓
PAPPL: IPP, queue, bounded complete PDF spooling
          ↓ small C ABI
letratag-printer: validation, job completion, journal, serialization
          ↓
letratag-render → letratag-protocol → letratag-transport
                                      ├─ Mock
                                      └─ bluer → BlueZ D-Bus (experimental)
```

* `letratag-protocol`: no Bluetooth/CUPS dependency, two source-backed
  profiles, validated headers/status packets, 32-row wire format.
* `letratag-render`: bounded PNG/grayscale input, proportional scaling,
  thresholding or Bayer dithering, pixel-exact PNG previews.
* `letratag-transport`: the same asynchronous interface for mock and
  BlueZ, timeouts, UUID/GATT checks, no uncontrolled retries.
* `letratag-printer`: optional PAPPL adapter, PDF through a resource-limited
  qpdf check and Poppler subprocess, full page validation before any transmission attempt.

[ADR](docs/adr/0001-print-system.md) · [Protocol and sources](docs/protocol.md) ·
[FFI contract](docs/ffi.md) · [Security](docs/security.md) ·
[Hardware acceptance](docs/hardware-acceptance.md) · [Licenses](docs/legal/README.md)

## Build

Rust 1.98.1 is pinned in `rust-toolchain.toml`. `Cargo.lock` is part of the
project. Native build dependencies: a C compiler, make, pkg-config,
libcups, Avahi, OpenSSL, libpng, libjpeg, zlib. The BLE feature also requires
the D-Bus development library. PDF support requires Poppler (`pdftoppm`)
plus qpdf and util-linux (`prlimit`) at runtime. Do not install them system-wide automatically.

Build only the hardware-independent Rust components:

```sh
cargo build --locked --workspace
cargo test --locked --workspace
```

Build PAPPL locally within the project, without root or a global installation:

```sh
bash scripts/build-pappl.sh
export PKG_CONFIG_PATH="$PWD/.research/pappl-prefix/lib/pkgconfig${PKG_CONFIG_PATH:+:$PKG_CONFIG_PATH}"
cargo build --locked --workspace --all-features
cargo test --locked --workspace --all-features
cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
cargo fmt --all -- --check
```

PAPPL 1.4.9 is built at the reviewed commit with the separately maintained
[document-policy patch](patches/README.md). Unpatched PAPPL is deliberately
rejected at compile time. The Rust version, Cargo dependency resolution,
PAPPL revision and patch are pinned; bit-identical binaries across different
system libraries are not promised. Rust 1.98 is the declared minimum.

## Preview and mock

```sh
# Fit a PNG/image proportionally into 400 feed columns and 30 active rows.
cargo run --locked -p letratag-printer -- preview input.png preview.png --columns 400

# Check provisional 50 × 12 mm PDF geometry without subsequent fit-to-page.
cargo run --locked -p letratag-printer -- preview label.pdf preview-pdf.png --pdf

# Mock endpoint, 127.0.0.1:8631 only, no Bluetooth or mDNS advertisement.
cargo run --locked -p letratag-printer --features pappl -- serve \
  --state "$PWD/test-output/local-journal" --spool "$PWD/test-output/local-spool" --port 8631
```

New state/spool directories are created with private permissions. Existing
directories must have mode 0700. Do not point them at `/etc/cups` or existing
CUPS spool directories. Preview does not overwrite existing files.

Print the CUPS integration settings and setup instructions (informational
only; nothing is installed or executed):

```sh
cargo run --locked -p letratag-printer -- cups-settings [--port 8631]
```

With the `ble` feature, read-only hardware diagnostics are available (no
print job is sent): a full GATT dump, state reads of the status
characteristics and an RX/AUX notification watch:

```sh
cargo run --locked -p letratag-printer --features ble -- hardware-info \
  --address '<device-address>' [--watch 15] [--set-cassette-type 3]
```

See [hardware acceptance](docs/hardware-acceptance.md) for the physical
print procedure and its preconditions.

Submit a test print through the running local PAPPL endpoint:

```sh
lp -h 127.0.0.1:8631 -d letratag \
  -o media=custom_default_12x50mm -o print-scaling=none label.pdf
```

This does not register a printer with the existing system CUPS. The separate
integration run starts and stops only its own process on an available high port:

```sh
bash scripts/test-ipp.sh
```

For a future system-wide queue, after separate approval: configure the IPP URI
`ipp://127.0.0.1:8631/ipp/print` as a new IPP destination. GUI discovery and the
path through a separate CUPS scheduler are separate acceptance checks.
Do not change an existing queue. A systemd template is in `packaging/systemd/`;
it is not installed or started automatically.

## Formats and limits

* Monochrome, one page and one copy per job. Use separate jobs rather than
  uncontrolled multiple prints.
* Variable length in the provisional IPP profile: 12 × 10..200 mm, default
  12 × 50 mm. `media-col` describes specific custom lengths. These limits are
  software/test profile values, not measured hardware capabilities.
* 200 dpi layout resolution, 30 active rows out of 32 wire rows. Print height,
  tape feed, and physical length are **not calibrated**.
* PDF: one 12 mm page, at most 200 mm long, with a provisional centered
  30-row content area. Nonwhite content outside this area causes an error.
  No silent cropping, stretching, or automatic fit-to-page.
* IPP accepts PDF only. PWG Raster and URF, including truncated headers and
  incomplete pages, are rejected before any submission. They are not enabled
  until complete-document validation can be demonstrated.
* PNG is supported by the CLI renderer, not the IPP endpoint. Images are fitted
  proportionally. PDF uses a separate geometry-preserving page path.
* No confirmed automatic cutting, no firmware updates, no persistent device
  changes, and no guarantee of QR code readability.

A timeout after the first write may mean that printing has already occurred.
The persistent inflight entry then blocks further transmissions, even after
a restart. Do not simply resubmit the job. See the procedure in
[security.md](docs/security.md).

## Future consumers

The intended path `label PDF → CUPS/IPP → Rust → BlueZ` contains no
Snipe-IT-specific logic. Notes on the existing 50 × 12 mm template are in
[pdf-integration.md](docs/pdf-integration.md). The reference project was not
modified. For now, a nine-page PDF must be split into individual pages/jobs;
it will not be printed partially.

## License

MIT for this project. Reference notices and license texts are in `docs/legal/`.
PAPPL and Poppler retain their own licenses. No AGPL Python code was ported;
Python is not required to build, test, or run this project.
