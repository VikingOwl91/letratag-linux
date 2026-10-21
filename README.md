# letratag-linux

Standalone Rust driver for the DYMO LetraTag 200B on Linux.
**Direct IPP is the recommended/default printing workflow.** PAPPL provides
the local IPP server; BlueZ is accessed over D-Bus. The optional system
[CUPS integration is Experimental](docs/cups-experimental.md).
No Python runtime, DYMO app, HomeOps, or Snipe-IT dependency.

**Development status: first physical print achieved and operator-confirmed
(2026-10-20, see [verification.md](docs/verification.md) and
[hardware-acceptance.md](docs/hardware-acceptance.md)).** The default IPP
destination is deliberately an explicitly named **mock destination**; the
BlueZ transport is wired into the same serve path as an explicit,
operator-selected option (`--transport bluez`). The rendering pipeline uses
a logical 200 x 200 DPI layout system mapped to the physical raster
geometry of the device (315 x 160 DPI, empirically calibrated and
physically validated for reference ink lengths from 10 mm to 100 mm, see
[calibration.md](docs/calibration.md) and the
[validation evidence](docs/evidence/hardware-validation-2026-10-21.md)).
That validation covers one device and one label; it is not a universal
dimensional-accuracy guarantee. The completion policy stays conservative:
the printer reports success even with the lid open, so no job is ever
auto-confirmed as physically printed. This is not a production printer
driver. See [verification.md](docs/verification.md) for the actual test
status.

## Architecture

```
Direct IPP client (ipptool / lp -h / lpr -H)
          ↓
PAPPL: IPP, queue, bounded complete PDF spooling
          ↓ small C ABI
letratag-printer: validation, job completion, journal, serialization
          ↓
letratag-render → letratag-protocol → letratag-transport
                                      ├─ Mock
                                      └─ bluer → BlueZ D-Bus (experimental)
```

Experimental CUPS adds a scheduler/PPD/PDF-filter stage before the same
PAPPL server. It is not required for the checked Direct IPP clients. General
desktop print-dialog support has not been verified; export a suitable PDF
and use the CLI workflow below.

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
[Hardware acceptance](docs/hardware-acceptance.md) ·
[Direct IPP guide](docs/direct-ipp.md) ·
[Experimental CUPS guide](docs/cups-experimental.md) ·
[crates.io plan](docs/publishing.md) · [Licenses](docs/legal/README.md)

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

## Quick start: Direct IPP (recommended/default)

After the PAPPL build above, start a private, mock-only endpoint. No system
CUPS scheduler, queue registration, default-printer change or Bluetooth is
needed. The `lp`, `lpr` and `ipptool` utilities are CUPS **clients**, and
libcups remains a native dependency; distribution package splits vary.

```sh
# Terminal 1: explicit mock transport, no physical printing.
target/debug/letratag-printer serve \
  --transport mock --profile legacy --capture --port 8631 \
  --state "$PWD/test-output/direct-journal" \
  --spool "$PWD/test-output/direct-spool"
```

From another terminal in the checkout:

```sh
# Query capabilities, then generate one synthetic 50 x 12 mm PDF.
ipptool -tv -T 5 ipp://127.0.0.1:8631/ipp/print tests/ipp/get-printer.test
mkdir -p test-output
rustc --edition=2024 tests/make-pdf.rs -o test-output/make-pdf
test-output/make-pdf > test-output/label-50.pdf

# Submit once, directly to PAPPL rather than the system scheduler.
lp -h 127.0.0.1:8631 -d letratag \
  -o media=custom_default_12x50mm -o print-scaling=none \
  test-output/label-50.pdf
```

Keep the explicit server and destination. Omitting `-h` can route the job
through system CUPS instead. For explicit dimensions or other supported
lengths, use the reusable request and a matching one-page PDF:

```sh
ipptool -tv -T 20 -d length=5000 -f test-output/label-50.pdf \
  ipp://127.0.0.1:8631/ipp/print examples/print-pdf.test
```

This is an **alternative** submission, not an additional step for the same
label. Submission success is not physical completion. The
[Direct IPP guide](docs/direct-ipp.md) covers job-state queries, explicit
63.5-mm media, troubleshooting, installation and evidence limits. The
software acceptance checks use only isolated mock servers:

```sh
bash scripts/test-direct-ipp.sh
bash scripts/test-ipp.sh
```

## Preview and mock rendering

```sh
# Fit a PNG/image proportionally into 400 feed columns and 30 active rows.
cargo run --locked -p letratag-printer -- preview input.png preview.png --columns 400

# Check 50 × 12 mm PDF geometry (calibrated physical raster) without subsequent fit-to-page.
cargo run --locked -p letratag-printer -- preview label.pdf preview-pdf.png --pdf

```

## Real hardware over BlueZ

Only after separate operator approval, the same Direct IPP serve path can
target the physical printer. This is an explicit
operator choice per invocation; there is never an implicit fallback from
BlueZ to mock, and a BlueZ job that fails is reported as failed/aborted,
never silently retried:

```sh
cargo run --locked -p letratag-printer --features pappl,ble -- serve \
  --state "$PWD/test-output/hw-journal" --spool "$PWD/test-output/hw-spool" \
  --port 8631 --transport bluez --address '90:E5:B1:49:DA:AE' \
  --profile legacy --chunk-mode auto --diagnostics
```

Requirements and behavior:

* The process user needs access to the system BlueZ D-Bus API (the same
  permission `bluetoothctl` uses). No pairing is required; the connection
  uses the explicit address. No persistent Bluetooth configuration is
  changed.
* Jobs are serialized: one transmission at a time, guarded by the engine
  mutex and the journal file lock.
* BLE write size is selected per `--chunk-mode` (see
  [protocol.md](docs/protocol.md) section 4): `auto` uses a
  hardware-validated capability recorded in `<state>/printer-caps.conf`
  and otherwise falls back to the conservative 20-byte limit; `safe`
  forces the conservative limit; `manual` takes `--max-write <bytes>`
  (experimental above the validated capability).
* Completion stays conservative: transport completion is not proof of a
  physical label. Every hardware job ends uncertain and keeps an inflight
  journal entry until the operator inspects the output and records a
  decision. See [hardware-acceptance.md](docs/hardware-acceptance.md).
* `--diagnostics` prints a content-free transport parameters block
  (write mode, negotiated ATT MTU, usable GATT value, validated cap,
  effective write size, chunk mode) plus per-write progress, raw status
  bytes and timing. It never logs print contents.

New state/spool directories are created with private permissions. Existing
directories must have mode 0700. Do not point them at `/etc/cups` or existing
CUPS spool directories. Preview does not overwrite existing files.

With the `ble` feature, read-only hardware diagnostics are available (no
print job is sent): a full GATT dump, state reads of the status
characteristics and an RX/AUX notification watch:

```sh
cargo run --locked -p letratag-printer --features ble -- hardware-info \
  --address '<device-address>' --watch 15
```

See [hardware acceptance](docs/hardware-acceptance.md) for the physical
print procedure and its preconditions.

## CUPS integration (Experimental, optional)

The existing CUPS integration remains available, but is not the default.
The confirmed `pdftopdf` media-fitting defect can shrink artwork on both
axes before PAPPL sees it. A submitted custom-media keyword does not prove
the selected or forwarded dimensions. The historical readable CUPS label
does not establish dimensional accuracy. An offline scaling-none control
is not a complete custom-media or hardware fix.

See the [Experimental CUPS guide](docs/cups-experimental.md) for preserved
setup commands, tested configurations, limitations and troubleshooting, and
the [forensic reference](docs/evidence/resolution-investigation-2026-10-21.md)
for the exact evidence. The existing informational command is retained:

```sh
cargo run --locked -p letratag-printer -- cups-settings --port 8631
```

A systemd template is in `packaging/systemd/`; it is not installed or
started automatically.

## Formats and limits

* Monochrome, one page and one copy per job. Use separate jobs rather than
  uncontrolled multiple prints.
* Variable length in the provisional IPP profile: 12 × 10..200 mm, default
  12 × 50 mm. `media-col` describes specific custom lengths. These limits are
  software/test profile values, not measured hardware capabilities.
* Documents are laid out in a **logical 200 × 200 DPI** coordinate system
  and rendered directly at the empirical physical raster geometry of the
  calibration profile (~315 DPI feed × ~160 DPI cross-tape, measured
  2026-10-21; estimates, not specifications — see
  [geometry.rs](crates/letratag-printer/src/geometry.rs) and
  [calibration.md](docs/calibration.md)). Override with `--calibration
  feed=<dpi>,cross=<dpi>`. IPP clients see the physical geometry in the
  standard `printer-resolution` attribute (the resolution at which the
  printer prints) and the logical layout resolution in the custom
  `letratag-logical-resolution` attribute.
* 30 active rows out of 32 wire rows, centered: the printable band is
  ~4.76 mm tall, below the 12 mm tape width. Nonwhite content outside the
  band is rejected, never silently cropped.
* PDF: one 12 mm page, at most 200 mm long, rendered at the physical
  raster resolution (no resampling, no automatic fit-to-page). Page
  rotation 90/270 is rejected with an actionable error.
* The physical calibration is empirical. It was physically validated once,
  for reference ink lengths from 10 mm to 100 mm on one device
  (2026-10-21); software tests verify the mapping, never physical
  dimensional accuracy on other devices or media.
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

The recommended path `label PDF → Direct IPP/PAPPL → Rust → BlueZ` contains no
Snipe-IT-specific logic. Notes on the existing 50 × 12 mm template are in
[pdf-integration.md](docs/pdf-integration.md). The reference project was not
modified. For now, a nine-page PDF must be split into individual pages/jobs;
it will not be printed partially.

## License

MIT for this project. Reference notices and license texts are in `docs/legal/`.
PAPPL and Poppler retain their own licenses. No AGPL Python code was ported;
Python is not required for the core build, IPP suites or driver runtime;
the optional forensic decoder/control tools use Python 3.
