# Verification record

## Current guidance, 2026-10-21

[Direct IPP](direct-ipp.md) is the recommended/default workflow. It sends
PDF directly to the shared PAPPL → Rust → BLE path, without a CUPS scheduler.
[CUPS is Experimental](cups-experimental.md); historical readable output
does not establish dimensional accuracy or a production-ready queue.
The CLI's transport default remains mock, not BlueZ. This update authorizes
no physical printing, hardware operation, queue changes or service activation.

The current evidence boundary is:

* Direct IPP 50 × 12 mm: one operator-confirmed readable physical label;
  detailed measurements were not taken.
* Direct IPP 50 × 12 mm and 63.5 × 12 mm: existing mock controls captured
  explicit media-col 1200 × 5000 and 1200 × 6350, yielding 315 and 400 columns.
  The longer direct control is not a physical acceptance result.
* Experimental CUPS 50 × 12 mm: historical readable-output acceptance only.
* CUPS `pdftopdf`: 0.787402 shrink on both axes reproduced offline after
  unresolved custom media selected a 50-mm output page. The submitted media
  keyword did not establish the actual size. Scaling none preserved the
  400-column artwork offline; that control is not a complete production fix.
  Historical scheduler-forwarded attributes and hardware payload bytes remain
  missing, as does full live scheduler-to-mock acceptance for this correction.
* The software still uses 160 dpi. Historical ~159/160-dpi text inference
  versus later conditional ~316-dpi apparent feed remains unresolved; native
  isotropic pitch and a DPI fix are not established.
* Hardware outcomes stay uncertain/inflight until operator inspection.
  `abort-job` is not a guarantee against every client/scheduler resend, and
  the journal is not cross-request idempotency. Current queue state has not
  been inspected here; do not infer removal from historical cleanup prose.

The authoritative [forensic report](evidence/resolution-investigation-2026-10-21.md)
retains measurements, hashes, controls, limits and its historical
execution-blocked CUPS shell regression. It is unchanged. Use the
[current hardware runbook](hardware-acceptance.md) for existing-hardware-first
prerequisites and conservative journal handling.

### Direct IPP recommendation checks, 2026-10-21

This update changes current documentation, informational `cups-settings`
output/CLI help, offline fixtures, and CI checks. No rendering, native DPI,
protocol, BlueZ, runtime configuration or version change was made. Direct
IPP was already available; no default routing change in production code
was necessary. `serve` still defaults to mock. The existing CUPS command and
integration remain available as Experimental.

Independent executing verification used staged Rust/Cargo 1.98.1, offline
vendored dependencies and the existing patched PAPPL 1.4.9 prefix. An initial
Rustup bootstrap failed with `Temporary failure in name resolution`; no
different toolchain was substituted. The existing same-version staged tools
and relocated prefix enabled the checks without downloading dependencies.

| Gate | Result |
|---|---|
| Default workspace tests, including doctests | 86 passed |
| All-feature workspace tests, including doctests | 105 passed |
| Workspace rustfmt and all-feature clippy `-D warnings` | Passed |
| General PDF-only IPP suite (`scripts/test-ipp.sh`) | Passed: rejection, direct lp/lpr, 4 mock captures, bounded history, multipage preservation |
| Final Direct IPP suite (`scripts/test-direct-ipp.sh`) | Passed: explicit 50/63.5-mm requests, lp/lpr, state queries, 4 independently decoded legacy mock captures |
| Standalone geometry oracle tests | 3 passed, including common shrink/shift/mirror, missing interior and extra ink rejection |
| Inflight-only completion-poll fixture | Count 0 while inflight, then 1 after completion; same helper used by actual polling |
| Targeted informational-report tests, default/all features | 3 passed in each configuration |
| Final standalone helper formatting and shell syntax | Passed |
| Local Markdown links | 65 file targets across 13 documents passed; URLs, anchors and historical evidence targets excluded |

The direct mock suite produced three 315 x 32 rasters and one 400 x 32
raster, each with the independently specified synthetic rectangle at
x44..266/y11..19. Only four antialiased corner pixels are excluded from its
mask oracle. The longer frame retains the identical artwork and adds 85
blank feed columns. A mismatched-length fifth job aborted with
`document-unprintable-error`, leaving only the original four engine journals.
Initial Print-Job acceptance was not treated as completion; the example
Get-Job request separately confirmed completed mock states.

The first standalone helper formatting check failed and was corrected.
Affected formatting, report tests, Direct IPP tests and printer clippy were
then re-executed successfully. The first combined Clippy run exceeded the
verification time limit; the isolated Clippy check passed. Default filtered
tests retain an existing warning: `function resolve_chunk_mode is never used`
at `crates/letratag-printer/src/main.rs:50`; all-feature Clippy is clean.

No requested local execution gate remains blocked. CI now includes the Direct
IPP suite and standalone-helper formatting, but no remote CI result is
claimed. Historical full CUPS scheduler/media-col replay, graphical dialogs,
packaged installation, systemd runtime and physical dimensional acceptance
remain unverified, not converted into passes by these checks.

The authoritative forensic SHA-256 remained
`8454d08866d45a45e31f7c1473e16225272717b4efa9bb2fd5ce64d7a55fc250`.
Tracked rendering, protocol, transport, native adapter, geometry and version
paths have no diff from the baseline. Tests used scratch-only builds and
explicit mock loopback destinations; no BLE connection, physical print,
system queue change, crate publication or release occurred.

### Release acceptance v0.3.0, 2026-10-21: software PASS, dimensional accuracy FAIL (NO-GO)

Phase 1 gates above were re-run and passed (86 default / 105 all-feature
tests, formatting, clippy, both IPP suites, forensic decoder, offline CUPS
geometry control, documentation links, scheduler-free Direct IPP
demonstration). Exactly two operator-approved physical prints were executed
through Direct IPP (normal 50 x 12 mm label; calibration label with
explicit media-col 1200 x 5000), both with preflight payload hashes, Write
Request at the validated 244-byte cap, legacy profile, write sequence
`[9,242,242,242,242,242,82]`, status `1b 52 00`, uncertain outcome by
design, and archived journals. Both labels were operator-confirmed
readable with correct orientation. The calibration rectangle measured
20.25 x 4.75 mm against a transmitted 252-dot x 30-row pattern expected at
40.0 x 4.76 mm in the 160-dpi layout: the feed direction is 49.4% short
(~316 dpi apparent), the across direction matches (~160 dpi), and the total
is 43.50 mm against ~53.4 mm expected. This reproduces the anomalous feed
pitch through Direct IPP, ruling out CUPS as its cause; the mechanism
remains unresolved and no DPI compensation was applied. Full evidence:
[release readiness report](evidence/release-readiness-v0.3.0-2026-10-21.md).
Recommendation: NO-GO for v0.3.0 with dimensional-accuracy claims. No tag,
publication or release was created.

### Addendum 2026-10-21 (btleplug BLE backend migration, phases 1-5)

A selectable `btleplug` BLE backend was added behind the existing
`Transport` abstraction (`BtleplugTransport` over a mockable
`GattCentral`/`GattPeripheral` boundary with a scripted mock), preserving
the BlueZ backend, the mock default, the Engine/journal semantics, the CLI
behavior, Direct IPP as recommended and CUPS as Experimental. Backend
selection: `serve --transport mock|bluez|btleplug`, `hardware-send
--transport bluez|btleplug` (default bluez). btleplug 0.11.8 is pinned
exactly; its Linux backend uses BlueZ via bluez-async/D-Bus.

Software gates after the change: default workspace 90 tests passed,
all-features 124 passed, zero failed; clippy `-D warnings` clean; rustfmt
clean; both IPP suites passed. New deterministic coverage: byte-exact
preservation of the retained HELLO LINUX and RAW diagnostic payloads
(committed fixtures), exact GATT operation traces with Write Request mode,
profile validation, discovery/status/write timeouts, mid-transfer failure
without replay, oversized-write rejection, short-command behavior, and
Engine journal semantics over the mock GATT boundary.

The connect-only backend comparison (no print payload transmitted) found
the same device, GATT table, characteristics, write mode (ATT Write
Request), payload hashes and effective write size (244) on both backends;
only connect/disconnect timings and the unexposed GATT value limit differ.
Evidence: [backend comparison](evidence/backend-comparison-2026-10-21.md),
[plan](btleplug-migration-plan.md). The physical feed-pitch discrepancy is
unchanged and not assumed fixed by the library change; hardware acceptance
(phase 6) is pending explicit operator authorization.

### Addendum 2026-10-21 (btleplug hardware acceptance and migration decision)

One operator-authorized HELLO LINUX print through the btleplug backend
(2026-10-21T05:54:10Z): transport btleplug, Write Request, validated cap
244, writes `[9,242,242,242,242,242,242,181]` identical to the BlueZ
reproduction, status `1b 52 00` at 9.91 s (7.16 s after last write),
uncertain outcome by design, journal archived. Operator-confirmed readable,
correct orientation; text width measured 12.3 mm — identical to the BlueZ
backend's current-era result. The library change alone does not resolve the
feed-pitch discrepancy; the BLE transport implementation is not a contributing
cause at the application/GATT-operation level. Decision: **retain both
backends; BlueZ remains the default**. Evidence:
[hardware acceptance and decision](evidence/btleplug-hardware-acceptance-2026-10-21.md).
No DPI, protocol or rendering change; no release, tag or publication.

### Addendum 2026-10-21 (calibrated 200-DPI logical rendering)

The original 2026-10-20 HELLO LINUX text-width measurement (~24.3 mm) was
corrected by the operator to 12.3 mm, consistent with all subsequent prints
across the Python/Bleak prototype and both Rust BLE backends. The pipeline
now separates a logical 200 x 200 DPI layout coordinate system (advertised
to IPP clients; exposed as `printer-resolution-supported = 200dpi`) from the
empirical physical raster geometry in a named, independently configurable
`CalibrationProfile` (default `LT200B_2026_10_21`: feed 315 / cross 160 DPI;
exposed to clients as the `letratag-physical-resolution` printer attribute).
PDFs render directly at the physical raster resolution (`pdftoppm
-scale-to-x/-scale-to-y`, exact anisotropic output, no resampling); the media
check uses the physical feed pitch; page rotation 90/270 is rejected with an
actionable error (Poppler distorts anisotropically scaled rotated pages).
`/Rotate 180` renders correctly. The PNG CLI path is unchanged
(physical-raster tool); the retained HELLO LINUX and RAW diagnostic fixtures
are byte-exact (transport crate `payload_preservation` tests untouched).

Gates: default workspace 90 + all-features 124 tests passed (12 new
geometry unit tests, 7 new document-path tests, 9 geometry integration
tests); rustfmt and clippy `-D warnings` clean; Direct IPP and PDF-only IPP
suites updated to the new physical geometry (50 mm -> 620 columns; 63.5 mm
-> 788 columns) and passing; forensic decoder and CUPS geometry controls
passing; documentation links pass. The calibration diagnostic
(`examples/calibration-label.rs --check`) verifies a 110 mm label with
10/20/40/50/100 mm reference lines offline. The v0.3.0
dimensional-accuracy NO-GO stands pending hardware validation
(docs/calibration.md). No release, tag or publication.

### Addendum 2026-10-21 (hardware validation: dimensional accuracy CONFIRMED)

One operator-authorized calibration label print via Direct IPP
(2026-10-21T06:56:19Z; 1364-column raster; 24 writes; status `1b 52 00`;
uncertain outcome by design; journal archived). Operator measurements:
reference lines 10 / 20 / 40 / 50 / 100 mm against commanded
9.9987 / 19.9975 / 39.9949 / 49.9937 / 99.9873 mm — residuals <= 0.013 mm,
within caliper precision, all implying 314.96 DPI feed (constant-DPI model
confirmed across 10-100 mm; no offset inferred). Band height 4.75 mm vs
4.7625 mm model (160 DPI cross confirmed). Lead-in 19 mm, lead-out ~9 mm,
total ~127 mm, consistent and recorded separately from ink geometry. The
default calibration profile is hardware-validated within measurement
precision. Evidence:
[hardware validation](evidence/hardware-validation-2026-10-21.md). The
v0.3.0 dimensional-accuracy NO-GO can be reconsidered on this evidence;
the operator decides. No release, tag or publication.

## Historical verification record begins, 2026-10-18

At that date the hardware-independent PDF-first candidate passed the build,
Rust and isolated mock IPP gates. No physical printer was available and no
BLE print was attempted then. The dated addenda record later results;
earlier calibration conclusions are superseded by the current guidance above.

## Levels

| Level | Evidence | Hardware proof? |
|---|---|---|
| Unit/property/golden tests | Rust test binaries | No |
| Mock integration | Complete Rust job and captured reference packets | No |
| Local print-system integration | Loopback PAPPL, IPP clients, lp/lpr | No |
| Physical hardware | Actual BLE exchange, tape output and measurements | Not executed as of 2026-10-18; later readable-output results below |

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
Applied patch SHA256 recorded for that run:
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
provisional 200 dpi to the then-inferred 160 dpi, the gates were re-run on the
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

### Addendum 2026-10-21 (crate READMEs and crates.io publication)

Dedicated README.md files were added for the three library crates
(commits `0208605`, `234fa00`, `68534e1`). Each README documents the
actual public API, purpose, installation, examples, features, limitations
and licensing, and links the main repository. Every README is included as
crate-level documentation via `#![doc = include_str!("../README.md")]`,
so each example is a doc-test compiled and run by `cargo test` (4 for
protocol, 3 for render, 5 for transport in both feature configurations);
each manifest references the README explicitly (`readme = "README.md"`)
and `cargo package --list` confirms it is packaged. Gates re-run on the
final state: default workspace 67 tests plus 12 doc-tests passed,
all-features 86 tests plus 12 doc-tests passed, zero failed; clippy with
`-D warnings` clean; rustfmt clean; debug and release builds passed; the
loopback IPP mock suite passed; license inventory passed.

Two of the three library crates were published to crates.io at 0.1.0 on
2026-10-21 (operator-approved): `letratag-protocol` and `letratag-render`,
both verified via the crates.io API and the sparse index.
**Correction:** `letratag-transport` was reported as published but is not
on crates.io (API 404, empty sparse-index entry, no `cargo search`
result); it was never uploaded. See the correction in
[publishing.md](publishing.md). `letratag-printer` remains unpublished by
design. The temporary publication token was left for the operator to
revoke; no credentials were committed (`.env` is git-ignored).

### Addendum 2026-10-21 (independent geometry forensics)

The final calibration's two-axis shrink is reproduced entirely offline:
CUPS `pdftopdf` embeds a 0.787402 scale into a 50-mm output PDF, yielding
315 raster columns instead of 400. Exact raster hashes agree between the
preserved CUPS mock capture and a new filter reconstruction. An offline
scaling-none control preserves 400 columns. No hardware payload bytes were
retained; hardware diagnostics establish lengths/timings only. This does
not prove variable printer stepping or resolve older apparent feed pitch.
The latest operator dimensions, chronology, independent decoder, hashes,
controls, and explicit uncertainties are recorded in
[evidence/resolution-investigation-2026-10-21.md](evidence/resolution-investigation-2026-10-21.md).
Production rendering, protocol, transport, and versions are unchanged.
The final inflight journal remains unarchived; the earlier draft's claim
that all four calibration journals were archived was incorrect. Earlier
Rust/build gates belong to their original changes, not proof of physical
geometry. New checks are offline decoder and CUPS-filter/mock controls.

### Addendum 2026-10-21 (CUPS end-to-end hardware acceptance)

The full CUPS path — `lp` → CUPS scheduler → IPP → PAPPL → Rust BLE
transport → BlueZ → DYMO LetraTag 200B — was acceptance-tested with one
physical print, operator-confirmed (label `CUPS-E2E-0001`, 50 × 12 mm).
A dedicated driverless queue `letratag-e2e` (`-m everywhere`,
`printer-error-policy=abort-job`) was created for the run; the historical
record reported its later removal and the existing `Canon_LBP633C` queue
untouched. This is not a current queue-state check. Before the
physical print, the identical CUPS path was validated against the mock
transport (temporary queue, since removed): one completed job, capture
decoded to width 315 with ink inside the printable band. Software-level
states diverge deliberately: the PAPPL endpoint reports the job aborted
(engine uncertain outcome, inflight journal), while CUPS reports
completed-successfully; neither is physical proof — the operator
confirmation is the acceptance criterion. No retries, no duplicate labels.
See [hardware-acceptance.md](hardware-acceptance.md) section 7.

### Addendum 2026-10-21 (0.1.1 patch publication completed)

After the operator updated the publication token, the README-carrying
0.1.1 patch packages were published in dependency order:
`letratag-protocol` 0.1.1, `letratag-render` 0.1.1, `letratag-transport`
0.1.1 (transport's first published version; 0.1.0 never existed). Each
publication was verified three ways: crates.io API (`max_version` 0.1.1),
the sparse index (0.1.1 present, not yanked), and an actual download of
the published `.crate` (README.md present, version 0.1.1). The published
`letratag-render` 0.1.1 lists `letratag-protocol ^0.1.1` as its
dependency. The 12 README doc-tests passed on the packaged state before
publication (default workspace 79 tests, all-features 98 tests, zero
failed; clippy `-D warnings` clean; rustfmt clean; debug and release
builds passed). A first 0.1.1 attempt was blocked by a token permission
error (HTTP 403); nothing was published in that attempt. The token was
left for the operator to revoke.

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

## Remaining acceptance boundaries (current, 2026-10-21)

* Terminal-status sequencing, auto-completion, reachable rows, minimum
  reliable feed and actual cut length remain open. Historical orientation
  observations do not settle the ~160 versus ~316 apparent feed conflict.
  Native pitch remains unverified; the historical ~3.4-mm extra lead-in is
  not modeled in the layout.
* The serve path supports `--transport bluez` behind an explicit operator
  choice; the mock remains the default. Direct IPP through that shared path
  produced readable 50 × 12-mm output without dimensional measurements.
  Direct 63.5-mm media-col acceptance remains mock-only.
* Direct PWG/URF input remains disabled until full-document validation is proven.
* PAPPL internal shutdown races are not repaired by the process-lifetime FFI
  workaround. Sanitizer testing and upstream follow-up remain necessary.
* Loopback is not per-user authentication. Native parsers and resource limits
  do not provide a complete hostile-document sandbox or a PDF correctness proof.
* Queue persistence, cancellation races, lost IPP acknowledgments and scheduler
  resubmissions need separate production acceptance. The durable side-effect
  journal is not a cross-request exactly-once service. See `security.md`.
* Experimental CUPS needs resolved forwarded media-col and post-filter
  geometry acceptance against mock; the offline scaling-none control alone
  is not a production fix. A graphical print dialog, systemd runtime, package
  installation and service-user BlueZ authorization remain separate checks.
  An attempted `cupsd -h` invocation was denied with `Permission denied`; it
  was not retried through another route.
* The existing HomeOps/Snipe-IT reference was only read. Its nine-page PDF is
  not an end-to-end driver acceptance fixture. The initial isolated run did
  not change an existing printer or global CUPS/Bluetooth configuration;
  later dedicated-queue experiments are separately recorded above.
