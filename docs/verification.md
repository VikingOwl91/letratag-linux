# Verification record

Status: the hardware-independent PDF-first candidate passed the build, Rust
and isolated mock IPP gates on 2026-10-18. Physical acceptance remains open.
No physical printer was available, and no BLE print was attempted.

## Levels

| Level | Evidence | Hardware proof? |
|---|---|---|
| Unit/property/golden tests | Rust test binaries | No |
| Mock integration | Complete Rust job and captured reference packets | No |
| Local print-system integration | Loopback PAPPL, IPP clients, lp/lpr | No |
| Physical hardware | Actual BLE exchange, tape output and measurements | Not executed |

## Final executed gates

An independent executing verifier used the exact staged Rust 1.98.1 toolchain,
locked vendored dependencies and the private PAPPL prefix in a read-only
`/work` mapping of the project. The final Rust/native gate run passed:

* Default workspace: 47 tests passed, zero failed.
* All features: 54 tests passed, zero failed.
* Clippy, all targets/features with `-D warnings`: passed.
* Rustfmt: passed.
* Debug and release all-feature workspace builds: passed.

PAPPL revision: `ae1889822d872eddd572a43ad0a4a3674fa5a9fa` (1.4.9).
Current applied patch SHA256:
`d9062ac7f3231c562eeecfe6c0ca52c7461ba607f05241e3af3b1e4685bad25a`.
All final gates used this patch, including the named CUPS-client URI amendment.
The clean reference checkout remains unchanged; the private library was built
from a separate patched copy. Retained upstream license files match their sources.

### Addendum 2026-10-20 (hardware bring-up changes)

After the BLE transport gained Write Request support (the verified LT200B TX
characteristic offers only the `write` flag) and the engine gained the
S1-based progress-ack handling plus a content-free diagnostic mode, the
offline gates were re-run on the final state: default workspace 50 tests
passed, all-features 60 tests passed, zero failed; clippy with
`-D warnings` clean; rustfmt clean; the `ble` feature binary builds. The
IPP script was not re-run; no PAPPL or CUPS configuration changed.

### Addendum 2026-10-20 (hardware acceptance achieved)

The first successful physical print was produced and operator-confirmed
(attempt 7; see
[evidence/hardware-attempt-7-2026-10-20-success.md](evidence/hardware-attempt-7-2026-10-20-success.md)).
After adding the short-command channel (set-cassette-type), the print-chunk
cap override and the post-status linger, the offline gates were re-run on
the final state: default workspace 53 tests passed, all-features 65 tests
passed, zero failed; clippy with `-D warnings` clean; rustfmt clean; the
`ble` feature binary builds; the IPP mock suite passed again after the
engine changes. Root cause of the earlier failed attempts: weak batteries.
Measured feed dot pitch: 158.9 dpi (manufacturer spec: 160 DPI). The
terminal-status completion policy is deliberately unchanged (hardware path
still reports uncertain and keeps an inflight journal) until calibrated
with repeated jobs and failure cases.

### Addendum 2026-10-21 (BLE serve path, chunk sizing, 160 dpi layout)

After wiring the BlueZ transport into the IPP serve path
(`serve --transport bluez --address <MAC>`, mock stays the default, no
fallback), adding adaptive chunk sizing (`auto`/`safe`/`manual` with
`effective_write_limit = min(transport, validated printer cap, protocol)`
and a per-address validated-caps file), documenting the job lifecycle with
recovery regression tests, and recalibrating the PDF layout from the
provisional 200 dpi to the measured 160 dpi, the gates were re-run on the
final state (commits `6c75613`..`eb2ba45`):

* Default workspace: 67 tests passed, zero failed.
* All features: 81 tests passed, zero failed.
* Clippy, all targets/features with `-D warnings`: passed.
* Rustfmt: passed.
* Debug and release all-feature workspace builds: passed.
* Dependency license inventory: passed.
* The loopback IPP mock suite passed again. All four independently
  decoded captures now have width 315 (50 mm at 160 dpi) and a 1288-byte
  body with SHA256
  `85a63e364d0efd3f141e01b2e477cabf059bcfaf82d4670507f5d463cd7d6c47`
  (was width 394, body 1604 bytes at the 200 dpi layout).
* CLI smoke checks: chunk-mode resolution errors (manual without
  `--max-write`, `--max-write` with auto/safe, out-of-range sizes),
  caps-file lookup and malformed-caps failure, and the per-job
  diagnostics parameters block through a live mock serve.

### Addendum 2026-10-21 (BLE connection lifecycle)

An independent, non-printing BlueZ lifecycle test on the physical printer
found that the device object is not guaranteed to exist before discovery
and that connection succeeds without pairing (see
[protocol.md](protocol.md) section 2). The transport gained bounded LE
discovery resolution for absent devices (`resolve_device`, 15 s / 256
events, exact address match) and reuse of an already-connected link;
`hardware-info` shares the same lifecycle. Offline gates re-run on the
final state (commits `6b03012`, `26227fc`): default workspace 67 tests
passed, all-features 86 tests passed, zero failed; clippy with
`-D warnings` clean; rustfmt clean; debug and release all-feature builds
passed; the loopback IPP mock suite passed; license inventory passed. The
new tests cover exact-address matching, stream end, event cap and timeout
bounding of the discovery wait, and the distinct actionable
`DeviceNotFound` error.

### Addendum 2026-10-21 (end-to-end hardware acceptance through the serve path)

The end-to-end acceptance print PDF → IPP/PAPPL → Rust → protocol → BlueZ →
DYMO LetraTag 200B was executed with exactly one attempt and produced an
operator-confirmed physical label (`ASSET-0001` / `DYMO LT200B`, 50 × 12 mm,
160 dpi layout, legacy profile, auto chunk sizing with the validated 244-byte
cap, 7 writes, single `1b 52 00` result 6.2 s after the last write, engine
outcome uncertain by design, journal archived with the operator decision).
Evidence:
[evidence/hardware-acceptance-serve-2026-10-21.md](evidence/hardware-acceptance-serve-2026-10-21.md).
A first submission the same day failed at connect with nothing transmitted
(lost BlueZ device cache, blueman-applet discovery interference, printer
auto-powered-off); the transport fix and the non-printing lifecycle test are
recorded in the addendum above. The physical result is operator-confirmed;
detailed ruler measurements of this label were not taken.

## Final isolated IPP results

The independent executing verifier ran the full script successfully, exit 0:

* 67 verbose IPP assertions passed, zero failed.
* Canonical and exact-name printer URIs worked; an unknown name returned
  `client-error-not-found` rather than selecting the default printer.
* All negative cases passed, with zero engine journals before valid submissions.
* Two ipptool jobs, one `lp` job and one `lpr` job completed through mock
  transport: four journals, four packet captures and four previews.
* The independent packet decoder validated all four captures. Each decoded
  body was 1604 bytes with SHA256
  `9d1fe3d34f7983a9cb80029e93489e9f093971c6f020502765a002fc641a743d`.
* The primary listener bound only to `127.0.0.1:38631`.
* Of 40 invalid flood submissions, 19 encountered `server-error-busy`.
  Retained metadata reached 32 records; completed mock output remained four jobs.
* The generic sink on port 38632 accepted the valid two-page PDF. Expected and
  received receipts matched: `RECEIVED 724 eb34ebef85ef53eb` (size and FNV-1a).
  The product's one-page policy separately rejected that document as a whole.

The run invoked ipptool 65 times, including four readiness attempts, plus
one `lp`, one `lpr` and two incomplete-HTTP checks. These are invocation counts,
not total wire-request counts. Runtime log output was filtered by the verifier;
passing gates do not establish warning-free runtime logs. Earlier runs reported
`Unable to initialize DNS-SD: Daemon not running`, without preventing loopback IPP.

The verifier inspected artifacts before its sandbox was discarded. The final
temporary capture path was
`/tmp/letratag-named-uri-ipp.kmu8GTTr/captures/ipp.IQYJpnfJ/`.
Those files are not retained in this repository. Counts and receipts here are
the executing verifier's recorded results, not a substitute for retained raw
logs. A local rerun retains its own logs under `test-output/ipp.*` by default.

## Failures found and corrected

The first IPP attempt stopped before server startup because a standalone
fixture was compiled with Rust's default edition and `TryInto` was not in
scope. The script now selects edition 2024 explicitly. That failure was a
test-helper compilation failure, not an IPP or hardware result.

The initial hand-written PDF fixture lacked a page Resources dictionary;
qpdf correctly rejected it with `Resources is missing or invalid; repairing`.
The fixture now declares an empty Resources dictionary and passes qpdf without
warnings. A subsequent capability test requested no explicit attributes but
expected `media-col-database`; PAPPL intentionally omits that large attribute
unless explicitly requested (`pappl/util.c:52`). The test now explicitly
requests each attribute it checks. Neither test failure justified changing
printer protocol values or starting system services.

The negative media-length test then found a genuine safety issue: PAPPL returned
`successful-ok-ignored-or-substituted-attributes (Ignoring unsupported media
keyword value.)` and completed one mock job after replacing an unsupported
media keyword with the default. No physical output occurred. The policy now
forces strict option fidelity, even if a client requests substitution. A
separate valid media collection tests PDF/media length disagreement without
relying on an unsupported keyword. The final rejection phase passed with zero
journals, including the genuine media-collection length mismatch and incomplete
HTTP bodies. The direct `lp` check then exposed CUPS's
named-destination `/printers/<name>` lookup, which PAPPL did not resolve.
The opt-in exact-name alias now covers that path; unknown names remain errors.
The final `lp`/`lpr` submissions and whole suite passed with that alias enabled.

Earlier test failures also exposed non-private default temporary directories;
test and PDF scratch directories now explicitly request mode 0700. The status
parser mismatch found in independent review was corrected to parse the complete
three-byte `ESC R code` notification, with prefix/length regression tests.

## Reproducible gates

```sh
bash scripts/build-pappl.sh
export PKG_CONFIG_PATH="$PWD/.research/pappl-prefix/lib/pkgconfig"
cargo test --locked --workspace
cargo test --locked --workspace --all-features
cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
cargo fmt --all -- --check
cargo build --locked --workspace --all-features
cargo build --locked --workspace --all-features --release
bash scripts/test-ipp.sh
```

The IPP script uses only synthetic documents and its own loopback processes.
It checks PDF-only capabilities, raster/URF rejection, truncated raster headers
including a truncated second header, incomplete pages, invalid/truncated PDFs,
oversized uploads, incomplete fixed-length/chunked HTTP requests, conflicting
media length, whole-job multipage rejection, lp/lpr submissions, independently
decoded mock packet captures, rejection-flood backpressure and generic
multipage byte preservation. An unexecuted assertion is not a passing test.

The raster cases prove rejection, not safe PWG/URF parsing: direct raster input
is disabled. The multipage receipt proves preservation for the synthetic valid
fixture, not universal PDF compatibility. This conservative MVP rejects valid
PDFs that emit qpdf warnings and rejects all multipage PDFs at product preflight.

The GitHub Actions workflow is configuration only until it runs on a forge.
No CI run, published release, package installation or service activation is
claimed by local results.

## Remaining acceptance boundaries

* Terminal-status sequencing, auto-completion, reachable rows, minimum
  reliable feed and actual cut length remain open. Row orientation and the
  effective DPI are measured (2026-10-20); the physical lead-in (~3.4 mm)
  is not modeled in the layout.
* The serve path supports `--transport bluez` behind an explicit operator
  choice; the mock remains the default. The serve-to-BLE path is wired and
  unit-tested but has not yet produced a physical print; hardware BLE
  evidence so far comes from the `hardware-send` path.
* Direct PWG/URF input remains disabled until full-document validation is proven.
* PAPPL internal shutdown races are not repaired by the process-lifetime FFI
  workaround. Sanitizer testing and upstream follow-up remain necessary.
* Loopback is not per-user authentication. Native parsers and resource limits
  do not provide a complete hostile-document sandbox or a PDF correctness proof.
* Queue persistence, cancellation races, lost IPP acknowledgments and scheduler
  resubmissions need separate production acceptance. The durable side-effect
  journal is not a cross-request exactly-once service. See `security.md`.
* A separate CUPS scheduler, graphical print dialog, systemd runtime, package
  installation and actual BlueZ authorization are separate acceptance checks.
  An attempted `cupsd -h` invocation was denied with `Permission denied`; it
  was not retried through another route.
* The existing HomeOps/Snipe-IT reference was only read. Its nine-page PDF is
  not an end-to-end driver acceptance fixture. No existing printer or global
  CUPS/Bluetooth configuration was changed.
