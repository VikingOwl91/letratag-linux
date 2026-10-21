# letratag-linux

Native Linux driver for the **DYMO LetraTag 200B** label printer. Render a
PDF label, print it over Bluetooth Low Energy, and expose the printer to any
IPP client.

**Latest release: [v0.3.0](https://github.com/VikingOwl91/letratag-linux/releases/tag/v0.3.0),
2026-10-21.**

The recommended way to print is **Direct IPP**. `letratag-printer serve` runs
a loopback-only IPP server, and `lp`, `lpr` or `ipptool` submit a PDF to it.
No system CUPS scheduler, queue registration or default-printer change is
needed. The optional system CUPS integration is
[Experimental](docs/cups-experimental.md).

Only the LetraTag 200B has been tested. The project is young: it has printed
real labels, but it is not a production printer driver, and it never claims
that a physical label came out of the printer.

## Features

* **Direct IPP printing.** A loopback IPP endpoint that accepts PDF only,
  one page and one copy per job, with explicit media selection so a
  submitted physical size is never silently refitted.
* **Rust implementation.** Protocol, renderer, BLE transports and the IPP
  service are Rust crates. No Python runtime is needed to build or run the
  driver.
* **Two BLE backends.** BlueZ over D-Bus is the default real-hardware
  backend; an optional [btleplug](docs/btleplug-migration-plan.md) backend is
  retained as an alternative.
* **Mock transport.** The printing workflow defaults to an in-process mock
  that records the exact bytes a job would send. The mock quick start and
  the mock test suites documented here never transmit to hardware. Real
  hardware always needs an explicit `--transport bluez` or
  `--transport btleplug`, and the read-only `hardware-info` command does
  connect to a device even though it never submits a print job.
* **Preview.** Render a PNG or a PDF to a wire-raster PNG without printing
  anything.
* **Calibrated geometry.** Documents are laid out in a logical 200 x 200 DPI
  coordinate system and rendered directly at the device's physical raster
  geometry (315 x 160 DPI), which was physically checked once for reference
  ink lengths from 10 mm to 100 mm. See
  [calibration.md](docs/calibration.md).

## Requirements

Hardware printing needs Linux with **BlueZ** and **D-Bus** at runtime. The
mock path needs neither. Build dependencies are listed with Debian/Ubuntu
package names, as used by CI:

| Purpose | Packages |
|---|---|
| Build the Rust code | `build-essential` `pkg-config` `git` |
| Build the bundled PAPPL | `libcups2-dev` `libavahi-client-dev` `libssl-dev` `libpng-dev` `libjpeg-dev` `zlib1g-dev` `libdbus-1-dev` |
| IPP clients (`lp`, `lpr`, `ipptool`) | `cups-client` `cups-ipp-utils` |
| PDF path at runtime | `poppler-utils` (`pdftoppm`) `qpdf` `util-linux` (`prlimit`) |
| Optional forensic decoder and docs check | `python3` |

The mock-only build compiles no Bluetooth stack: with the `ble` and
`btleplug` features off, neither `bluer` nor `btleplug` is compiled in.
`libdbus-1-dev` is still needed, because the bundled PAPPL requires
D-Bus.

Rust 1.98.1 is pinned in `rust-toolchain.toml`; 1.98 is the declared
minimum. `Cargo.lock` is part of the project.

## Installation

### Build from source

The IPP service links against PAPPL 1.4.9 with a separately maintained
[document-policy patch](patches/README.md), which the project builds into a
private prefix. Unpatched PAPPL is deliberately rejected at compile time.

```sh
git clone https://github.com/VikingOwl91/letratag-linux.git
cd letratag-linux

bash scripts/build-pappl.sh
export PKG_CONFIG_PATH="$PWD/.research/pappl-prefix/lib/pkgconfig${PKG_CONFIG_PATH:+:$PKG_CONFIG_PATH}"

# Mock only: no Bluetooth backend, no Bluetooth development libraries.
cargo build --locked --release -p letratag-printer --features pappl
```

The BLE backends are separate Cargo features, so add the one you need only
if you intend to print on real hardware:

```sh
# BlueZ over D-Bus, the default real-hardware backend.
cargo build --locked --release -p letratag-printer --features pappl,ble

# The optional btleplug backend.
cargo build --locked --release -p letratag-printer --features pappl,btleplug
```

A binary built without a BLE feature refuses that transport at startup and
names the feature to build. The Rust version, dependency resolution, PAPPL
revision and patch are all pinned. Bit-identical binaries across different
system libraries are not promised.

### Arch Linux

`packaging/arch/PKGBUILD` builds an Arch package from the release source
archive. It is **not** in the AUR and no AUR submission exists. The recipe
pins the checksum of the release archive, so fetch that archive into the
build directory first:

```sh
cd packaging/arch
curl -L -O https://github.com/VikingOwl91/letratag-linux/releases/download/v0.3.0/letratag-linux-0.3.0.tar
makepkg --verifysource
makepkg
```

`makepkg --verifysource` fails if the archive is missing or does not match
the pinned SHA256. The recipe builds the patched PAPPL from the pinned
upstream tarball and links it statically, so the installed package has no
runtime PAPPL dependency. See [packaging/README.md](packaging/README.md).

A systemd unit template is in `packaging/systemd/`. It is mock-only, it is
not installed by the project, and it is never started automatically.

## Quick start: a safe first print

The steps below use the mock transport, so no Bluetooth transport is
selected and nothing can reach a printer. The mock records the bytes a real
job would send. Real hardware needs an explicit `--transport bluez` or
`--transport btleplug`; see [Printing on real hardware](#printing-on-real-hardware).

```sh
# 1. Build the mock-only binary (see Installation above for the PAPPL prefix).
cargo build --locked --release -p letratag-printer --features pappl

# 2. Start a private, mock-only IPP endpoint. No Bluetooth, no system CUPS.
target/release/letratag-printer serve \
  --transport mock --profile legacy --capture --port 8631 \
  --state "$PWD/test-output/journal" \
  --spool "$PWD/test-output/spool"
```

In a second terminal, in the same checkout:

```sh
# 3. Make one 50 x 12 mm PDF and check the endpoint's capabilities.
mkdir -p test-output
rustc --edition=2024 tests/make-pdf.rs -o test-output/make-pdf
test-output/make-pdf > test-output/label-50.pdf
ipptool -tv -T 5 ipp://127.0.0.1:8631/ipp/print tests/ipp/get-printer.test

# 4. Submit the label. -h sends it to our endpoint, not to a system queue.
lp -h 127.0.0.1:8631 -d letratag \
  -o media=custom_default_12x50mm -o print-scaling=none \
  test-output/label-50.pdf

# 5. The mock recorded the exact wire payload, a PNG preview and the
#    journal entry for the job:
ls test-output/journal/
```

Keep the explicit server and destination: omitting `-h` can route the job
through a system CUPS scheduler instead. `print-scaling=none` is the only
supported scaling mode; the media keyword must match the PDF page.

Other lengths use the same `media-col` mechanism (see the
[Direct IPP guide](docs/direct-ipp.md)), and the reusable IPP request
`examples/print-pdf.test` submits a PDF with an explicit length.

To look at a rendering without any server, use `preview`:

```sh
cargo run --locked -p letratag-printer -- preview input.png preview.png --columns 400
cargo run --locked -p letratag-printer -- preview label.pdf preview-pdf.png --pdf
```

The software acceptance checks use only isolated mock servers:

```sh
bash scripts/test-direct-ipp.sh
bash scripts/test-ipp.sh
```

Those two suites start their own mock servers and never select a BLE
transport. Do not assume that an arbitrary command or script is
hardware-safe: read its options, and treat anything that takes a
`--transport` other than `mock` as a hardware operation.

## Printing on real hardware

> **Warning.** The printer reports success even with the lid open, so a
> successful job does not prove that a label was printed. Every hardware job
> ends *uncertain* and keeps an inflight journal entry until an operator
> inspects the output and records a decision. A timeout after the first
> write may mean that the label was already printed: the journal then blocks
> further transmissions, even after a restart. **Do not resubmit the job.**

Hardware is always an explicit, per-invocation choice. There is no implicit
fallback from BlueZ to mock, and a failed hardware job is reported as
failed, never silently retried.

The mock-only binary from the quick start has no BLE backend. Build the one
you need first:

```sh
# BlueZ over D-Bus, the default real-hardware backend.
cargo build --locked --release -p letratag-printer --features pappl,ble

# Or the optional btleplug backend.
cargo build --locked --release -p letratag-printer --features pappl,btleplug
```

Then start the endpoint with the matching transport:

```sh
# Replace <device-address> with your printer's Bluetooth address.
target/release/letratag-printer serve \
  --transport bluez --address '<device-address>' \
  --profile legacy --chunk-mode auto --diagnostics \
  --state "$PWD/test-output/hw-journal" --spool "$PWD/test-output/hw-spool" \
  --port 8631
```

`<device-address>` is a placeholder and the command will not run as written.
Find the printer's address with the standard BlueZ tools, for example:

```sh
bluetoothctl scan on      # power the printer on and wait for it to appear
bluetoothctl devices      # lists the known addresses and device names
bluetoothctl info <address>   # confirms which device an address belongs to
```

The address is the MAC-style identifier shown by `bluetoothctl devices`. The
project's own `hardware-info` command needs the address as input, so it
cannot be used to find one.

* The process user needs access to the system BlueZ D-Bus API, the same
  permission `bluetoothctl` uses. No pairing is required and no persistent
  Bluetooth configuration is changed.
* `--transport btleplug` selects the alternative btleplug backend. It uses
  the same wire protocol, write mode and chunking, and was accepted on
  hardware; BlueZ stays the default.
* `--profile legacy` is the profile that was validated on hardware. The
  profile is never switched automatically.
* Jobs are serialized: one transmission at a time, guarded by the engine
  mutex and the journal lock.
* `--chunk-mode auto` uses a hardware-validated write size recorded per
  device in `<state>/printer-caps.conf` and otherwise falls back to the
  conservative 20-byte limit. `safe` forces the conservative limit and
  `manual` takes `--max-write <bytes>`.
* `--diagnostics` prints a content-free transport parameter block and
  per-write progress. It never logs print contents.

New state and spool directories are created with private permissions;
existing ones must be mode 0700. Do not point them at `/etc/cups` or an
existing CUPS spool directory.

Read-only hardware inspection is available separately. It connects to the
device and reads its GATT characteristics, but never sends a print job, and
it needs the same BLE-enabled build:

```sh
target/release/letratag-printer hardware-info --address '<device-address>' --watch 15
```

The full physical print procedure, its preconditions and the journal
handling rules are in the
[hardware acceptance guide](docs/hardware-acceptance.md).

## Known limitations

* **Direct IPP is the recommended path.** The CUPS integration is
  Experimental: the `pdftopdf` filter can fit artwork into the wrong page
  before the printer sees it, and a submitted media keyword does not prove
  the size that was actually used.
* **One tested printer model.** Only the DYMO LetraTag 200B. No other
  model and no other operating system is claimed.
* **No universal dimensional accuracy.** The physical calibration was
  checked once, on one device, for reference ink lengths from 10 mm to
  100 mm, with the measurements recorded to the nearest millimetre. That is
  not a guarantee for other devices, media or input paths.
* **No automatic confirmation of a physical print.** Every hardware job
  ends uncertain, by design.
* **No automatic cutting guarantee.** The printer's own lead-in and
  lead-out are outside the calibration.
* **PDF only over IPP.** PWG Raster and URF are rejected before any
  submission. The CLI renderer also accepts PNG.
* **One page, one copy, monochrome.** A multi-page PDF is rejected; split it
  into separate jobs.
* **Page rotation 90° and 270° is rejected** with an actionable error.
  Export the page pre-rotated.
* **The printable band is 4.76 mm high** inside the 12 mm tape width.
  Content outside the band is rejected, never cropped.

## Documentation

| Topic | Document |
|---|---|
| User guide: installing, serving, media, troubleshooting | [docs/direct-ipp.md](docs/direct-ipp.md) |
| Experimental CUPS integration and its known defects | [docs/cups-experimental.md](docs/cups-experimental.md) |
| Physical print procedure and journal handling | [docs/hardware-acceptance.md](docs/hardware-acceptance.md) |
| Calibrating the physical geometry | [docs/calibration.md](docs/calibration.md) |
| Security model and operational safety | [docs/security.md](docs/security.md) |
| Wire protocol, profiles and sources | [docs/protocol.md](docs/protocol.md) |
| Rendering architecture and the calibration model | [docs/rendering-architecture-200dpi.md](docs/rendering-architecture-200dpi.md) |
| C/Rust boundary contract | [docs/ffi.md](docs/ffi.md) |
| Print system decision record | [docs/adr/0001-print-system.md](docs/adr/0001-print-system.md) |
| Template and PDF notes for existing integrations | [docs/pdf-integration.md](docs/pdf-integration.md) |
| Test status and the dated verification record | [docs/verification.md](docs/verification.md) |
| Release notes and changelog | [docs/releases/v0.3.0.md](docs/releases/v0.3.0.md), [CHANGELOG.md](CHANGELOG.md) |
| Packaging and publication | [packaging/README.md](packaging/README.md), [docs/publishing.md](docs/publishing.md) |
| Licenses, attribution and protocol provenance | [docs/legal/README.md](docs/legal/README.md) |

The `docs/evidence/` directory keeps the dated investigation and acceptance
records, including the
[hardware validation](docs/evidence/hardware-validation-2026-10-21.md) and
the [geometry forensics](docs/evidence/resolution-investigation-2026-10-21.md).
They are the record of what was measured, including measurements that were
later corrected.

## Contributing, license and attribution

The project has no contribution guide yet. For design questions, read the
[decision record](docs/adr/0001-print-system.md); for defects and questions,
use the repository's issue tracker. Please keep the historical evidence
documents intact: they are the record of the measurements behind the current
model.

Project code is MIT licensed, see [LICENSE](LICENSE). PAPPL and Poppler keep
their own licenses, and the protocol work credits its sources in
[docs/legal/README.md](docs/legal/README.md). No DYMO code or assets are
redistributed.
