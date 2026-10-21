# Direct IPP (recommended/default workflow)

Send a complete label PDF directly to the local **PAPPL** IPP server.
PAPPL spools and queues it; the existing Rust renderer, protocol encoder,
and selected mock or BlueZ transport process the job. No system CUPS queue,
PPD, or CUPS scheduler filter is involved in this route.

This recommendation does not change the safe transport default: `serve`
still defaults to **mock**. Physical printing requires explicit BLE backend
selection (BlueZ default, btleplug alternative) and operator approval.
Direct IPP avoids the demonstrated
[Experimental CUPS fitting issue](cups-experimental.md); the rendering
pipeline models the printer's empirical physical geometry via the
[calibration profile](calibration.md).

## Build and local setup

These commands assume a source checkout and the native dependencies listed
in the [README](../README.md#build). There is no automatic package, service,
queue, or Bluetooth-policy installation. `lp`, `lpr`, and `ipptool` are
CUPS client utilities; their package names vary by distribution. Using those
clients directly does **not** require running the system CUPS scheduler.
The build still requires libcups and the pinned patched PAPPL.

```sh
bash scripts/build-pappl.sh
export PKG_CONFIG_PATH="$PWD/.research/pappl-prefix/lib/pkgconfig${PKG_CONFIG_PATH:+:$PKG_CONFIG_PATH}"
cargo build --locked --workspace --all-features
```

Use private, previously unused state/spool paths. Existing directories must
be mode 0700. Keep unresolved journals; do not point the server at a system
CUPS spool. Check that port 8631 is available before starting it.

```sh
# Terminal 1: no Bluetooth. This is the recommended first setup check.
target/debug/letratag-printer serve \
  --transport mock --profile legacy --capture --port 8631 \
  --state "$PWD/test-output/direct-journal" \
  --spool "$PWD/test-output/direct-spool"
```

The server binds only `127.0.0.1`; it does not advertise through mDNS.
In another terminal, from the checkout:

```sh
ipptool -tv -T 5 ipp://127.0.0.1:8631/ipp/print tests/ipp/get-printer.test
```

This is a capability query, not a print. The URI is
`ipp://127.0.0.1:8631/ipp/print`; the named destination for direct `lp`/`lpr`
clients is `letratag`. No `lpadmin` command or system default-printer change
is needed. Support for arbitrary desktop application print dialogs is not
verified; export a suitable one-page PDF and use the checked CLI route.

## Prepare and submit a PDF

To generate the same synthetic 50 x 12 mm page used in the mock tests:

```sh
mkdir -p test-output
rustc --edition=2024 tests/make-pdf.rs -o test-output/make-pdf
test-output/make-pdf > test-output/label-50.pdf
target/debug/letratag-printer preview \
  test-output/label-50.pdf test-output/direct-preview.png --pdf
```

Preview refuses to overwrite an existing file. Check that your own document
meets the contract below before substituting it for the synthetic label.

Submit exactly once to the running **mock** server:

```sh
lp -h 127.0.0.1:8631 -d letratag \
  -o media=custom_default_12x50mm -o print-scaling=none \
  test-output/label-50.pdf
```

Alternatively, use `lpr` with the same explicit server and settings, not in
addition to the `lp` submission:

```sh
lpr -H 127.0.0.1:8631 -P letratag \
  -o media=custom_default_12x50mm -o print-scaling=none \
  test-output/label-50.pdf
```

Omitting `-h`/`-H` can route to the system CUPS scheduler instead. `lp -d
letratag-e2e ...` is the separate Experimental CUPS workflow, not Direct IPP.

### Explicit media dimensions, including other lengths

For explicit media use the reusable `examples/print-pdf.test` request.
`length` is the feed length in **1/100 mm**, not raster pixels. The request
sets `application/pdf`, one copy, scaling none, zero margins, and a media
collection with `x-dimension=1200`, `y-dimension=$length`.

```sh
# Alternative single submission of a 50 x 12 mm PDF:
ipptool -tv -T 20 -d length=5000 -f test-output/label-50.pdf \
  ipp://127.0.0.1:8631/ipp/print examples/print-pdf.test

# A separate example: 180 pt feed = 63.5 mm. Artwork coordinates stay fixed.
test-output/make-pdf 180 > test-output/label-63.5.pdf
ipptool -tv -T 20 -d length=6350 -f test-output/label-63.5.pdf \
  ipp://127.0.0.1:8631/ipp/print examples/print-pdf.test
```

These are independent jobs. Never submit both alternatives to obtain one
label. Software media range is 12 x 10..200 mm, default 12 x 50 mm. A PDF's
feed length must match the requested length within raster rounding; there
is no automatic fit to media. The 63.5-mm route is mock-tested, **not
physically verified**. Do not infer support for arbitrary media keywords
or for every length from the two tested lengths. A media-col is preferable
to inventing a custom keyword.

### Status and uncertain outcomes

Submission success means accepted/queued, not physically printed. Take the
numeric job ID from the response (for `lp`, the suffix of `letratag-N`):

```sh
# Replace 1 with the returned job ID. This only queries; it does not resubmit.
ipptool -tv -T 5 -d jobid=1 \
  ipp://127.0.0.1:8631/ipp/print examples/get-job.test
```

Mock jobs complete without Bluetooth. A hardware transmission deliberately
ends uncertain, with an inflight journal and an aborted endpoint job,
because printer status `1b 52 00` also occurs without physical output.
A lost response or timeout after transmission may conceal a printed label.
Do not automatically retry, delete a journal, or submit a replacement job.
Inspect output and follow the [journal recovery procedure](security.md).
After an operator-authorized archive, restart the server: it also keeps an
in-memory blocked flag.

## Real hardware is a separate, explicit choice

No hardware print is part of installation or the tests above. Only after
operator approval, stop the mock server and start the same PAPPL service
with the established BLE options and the printer's own private journal:

```sh
target/debug/letratag-printer serve \
  --state "$PWD/test-output/hw-journal" --spool "$PWD/test-output/hw-spool" \
  --port 8631 --transport bluez --address '<device-address>' \
  --profile legacy --chunk-mode auto --diagnostics
```

`--transport bluez` is the established backend (build with `pappl,ble`).
`--transport btleplug` selects the alternative btleplug backend (build with
`pappl,btleplug`); see the
[migration plan](btleplug-migration-plan.md) and the
[backend comparison](evidence/backend-comparison-2026-10-21.md). Both
backends use the same wire protocol, write mode and chunking. The service
user needs BlueZ/D-Bus permission.
Use only an established printer capability in `printer-caps.conf`; auto
otherwise uses its existing conservative fallback. Do not change chunking
or protocol settings to compensate for geometry. Follow the complete
[hardware runbook](hardware-acceptance.md), including readiness, one-job
approval, output inspection and conservative journaling. The print commands
are the same Direct IPP clients; the selected transport determines whether
they consume physical tape.

## Document contract and evidence limits

- PDF only, one page and one copy, up to the existing 8-MiB job limit.
  PNG is CLI-preview/mock input, not an IPP document format. PWG/URF are rejected.
- One 12-mm page, at most 200 mm feed length, in landscape or corresponding
  portrait orientation. Leave orientation-requested unset and use scaling none.
  Page rotation 90/270 is rejected with an actionable error (export the page
  pre-rotated).
- Documents are laid out in a **logical 200 x 200 DPI** coordinate system.
  The pipeline renders directly at the physical raster geometry of the
  [calibration profile](calibration.md) (~315 DPI feed x ~160 DPI cross-tape
  by default; empirical estimates, not specifications). IPP clients see the
  physical geometry in the standard `printer-resolution` attribute (the
  resolution at which the printer prints) and the logical layout resolution
  in the custom `letratag-logical-resolution` attribute. A 50 x 12 mm page
  renders to 620 feed columns x 76 cross-tape pixels at the default profile.
  Override with `--calibration feed=<dpi>,cross=<dpi>`.
- The centered 30-row printable band is ~4.76 mm high (at 160 DPI
  cross-tape), below the 12 mm tape width. Nonwhite content outside that
  band is rejected, not cropped or squeezed into it.
- The media check compares the native raster width against the media length
  through the physical feed pitch; page fitting cannot override requested
  physical dimensions, and inconsistent media aborts with an actionable error.
- Current software limits are not hardware capability or dimensional-accuracy
  guarantees. The calibration profile is empirical. It was physically
  validated once, for reference ink lengths from 10 mm to 100 mm on one
  device (2026-10-21, see
  [hardware validation](evidence/hardware-validation-2026-10-21.md)); that
  evidence covers the calibrated Direct IPP path only.
- Historical physical Direct IPP acceptance established a readable 50 x 12 mm
  source label through PAPPL/BlueZ, not measured physical accuracy. Mock
  controls establish the checked document/attribute/raster behavior only.

## Troubleshooting and acceptance

| Symptom | Check without blindly resubmitting |
|---|---|
| Capability query cannot connect | Server process, explicit port/URI, loopback binding; no system queue is needed |
| `lp` cannot find a destination | Keep both `-h 127.0.0.1:8631` and `-d letratag`; query capabilities first |
| Job aborts before transport | One clean PDF page, matching explicit dimensions, scaling none, centered ink band and supported options |
| Hardware job aborts after transmission | Treat as uncertain; retain journal and inspect tape |
| Journal is inflight / server blocked | Use the operator recovery procedure, then restart; do not delete evidence |
| Printed dimensions differ | Stop physical activity; compare received document and decoded raster to the forensic reference; do not tune DPI |

Run the documentation-oriented client/media regression and the broader
PDF-only receive-policy suite without a printer:

```sh
bash scripts/test-direct-ipp.sh
bash scripts/test-ipp.sh
```

They use isolated high-port PAPPL **mock** servers and no system CUPS queue.
The direct regression checks `lp`, `lpr`, explicit 50/63.5-mm IPP requests,
job queries, mismatched-length rejection, and independently decoded legacy
rasters with unchanged artwork. Hardware acceptance is separate and needs
new explicit approval; none is authorized by this document.
