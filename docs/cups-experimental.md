# CUPS integration (Experimental)

**Direct IPP is the recommended/default workflow.** See
[setup and printing](direct-ipp.md). CUPS remains available as an optional
system-scheduler integration with the same PAPPL server and unchanged Rust
rendering/protocol/BlueZ architecture. It is not required for the verified
command-line clients. Desktop print-dialog compatibility is not generally
verified.

## Confirmed media-fitting defect

The authoritative record is
[resolution-investigation-2026-10-21.md](evidence/resolution-investigation-2026-10-21.md).
With the investigated PPD/filter stack, a 63.5 x 12 mm source PDF submitted
as `media=custom_12x63.5mm` was fitted by `pdftopdf` into a 12 x 50 mm page.
The filter embedded a **0.787402 scale on both artwork axes**, before the
document reached PAPPL. Direct rendering kept a 400-column frame and
252-pixel rectangle edge/30 occupied rows; the filtered reproduction had
315 columns, a 199-pixel edge and 24 occupied rows. This explains the
approximately315/400 two-axis physical shrink without a new feed mode.

A CUPS job retaining the submitted media keyword does **not** establish
the media dimensions selected by the PPD, written into the PDF, or
forwarded in IPP. Inspect actual document geometry and media-col, not just
the keyword. PAPPL's scaling-none default cannot undo fitting already
embedded in its input PDF. CUPS job completion and readable physical output
do not establish dimensional accuracy.

### Tested configurations and limits

| Configuration | Evidence | Limit |
|---|---|---|
| 50 x 12 mm source, named `12x50mm.Borderless` | Historical readable physical label; offline filter retains 252-pixel edge/30 rows | Physical dimensions were not measured for that acceptance label; no general accuracy guarantee |
| 63.5 x 12 mm source, `custom_12x63.5mm`, scaling option omitted | Offline reproduction matches the preserved CUPS mock raster exactly; hardware write lengths support 315 columns | Demonstrated unwanted fitting; not suitable as a dimensional workflow |
| Same failing filter invocation plus `print-scaling=none` | Offline control preserves the 400-column source raster | Not a complete scheduler/media-col or hardware fix; a mismatched forwarded length can cause rejection |
| `Custom.12x63.5mm` alone | Selected a custom size in a libppd probe, but the filter control produced A4 and the renderer rejected it | Renaming the option alone is not a verified workaround |

No fully verified custom-length CUPS workaround is claimed. Use Direct IPP
with explicit media-col for the checked software route instead. The
160-versus-316-dpi apparent feed discrepancy of the historical hardware
evidence was resolved by the 2026-10-21 calibration and hardware
validation; that work validated the Direct IPP path, not the CUPS
scheduler path, so the CUPS limitations listed above stand. Do not change
native DPI, introduce anisotropic scaling, or adjust BLE settings to
compensate.

Actual historical hardware payload bytes and scheduler-forwarded media-col
were not retained. The forensic investigation's full scheduler replay was
blocked by permissions; the filter and Direct IPP/PAPPL controls were
performed offline. Keep those evidence limits when interpreting this guide.

## Optional setup, not an installation requirement

Changing system queues needs explicit operator authorization. Never replace
an existing printer. Start with a dedicated **mock-only** PAPPL server and
verify it answers before creating a dedicated queue:

```sh
# Terminal 1, source checkout after the normal build. No Bluetooth.
target/debug/letratag-printer serve \
  --transport mock --profile legacy --capture --port 8631 \
  --state "$PWD/test-output/cups-mock-journal" \
  --spool "$PWD/test-output/cups-mock-spool"

# Terminal 2: query before any authorized queue registration.
ipptool -tv -T 5 ipp://127.0.0.1:8631/ipp/print tests/ipp/get-printer.test

# Only if separately authorized. The name must not already exist.
lpadmin -p letratag-e2e -E -v ipp://127.0.0.1:8631/ipp/print -m everywhere
lpadmin -p letratag-e2e -o printer-error-policy=abort-job
lpoptions -p letratag-e2e -l

# The historical 50-mm configuration, against the mock endpoint only.
lp -d letratag-e2e -o media=12x50mm.Borderless label.pdf
lpstat -W all -o letratag-e2e

# Remove only the dedicated test queue, if its removal was authorized.
lpadmin -x letratag-e2e
```

These preserve the existing integration; they are not a newly verified
system-queue acceptance run. `cups-settings` remains available as an
informational report, now explicitly marking CUPS Experimental.

For a dimensional acceptance candidate, first inspect the **post-filter**
PDF/raster and actual forwarded IPP dimensions against a mock endpoint.
Check orientation, margins, both artwork axes and frame length. A source
preview alone is insufficient. No hardware test or queue modification is
authorized merely by reading these examples.

## Failure handling and troubleshooting

- `lpadmin: Unauthorized`: stop and ask the operator to perform authorized
  administration. Do not bypass the denial or run elevated commands here.
- A failed queue registration can also mean the endpoint is unavailable;
  check the read-only capability query before diagnosing permissions.
- Pending jobs can indicate a stopped endpoint or disabled queue. Inspect
  state before enabling it: enabling a queue can send pending jobs.
- `abort-job` is a conservative queue policy, not a guarantee against every
  scheduler, client, lost-acknowledgment, or manual resend scenario.
- CUPS can report completed while PAPPL reports aborted/uncertain. Neither
  alone proves a physical label. Check the inflight journal and output,
  following [security/recovery](security.md); do not retry blindly.
- A smaller rectangle on unchanged cut length can originate in the filtered
  artwork. Preserve documents, IPP attributes, captures and logs before
  proposing hardware explanations or a corrective scaling factor.

Offline CUPS-filter controls are available in
`scripts/test-cups-geometry.sh`. They do not register a queue or send to
hardware. A passing filter control is not full scheduler or physical
acceptance. Direct IPP acceptance and the historical CUPS-readable-output
record are kept separate in [hardware acceptance](hardware-acceptance.md).
