# RAW raster diagnostic, 2026-10-21: feed pitch measured without any document pipeline

## Purpose and method

Eliminate PDF, Poppler, PNG, CUPS, IPP, PAPPL and all document scaling from
the print path to independently measure the physical feed pitch of the
LetraTag 200B (`90:E5:B1:49:DA:AE`).

The diagnostic is a cargo example,
`crates/letratag-printer/examples/raw-raster-diagnostic.rs`, run through
`scripts/raw-raster-diagnostic.sh`. It builds a deterministic 1-bit raster
entirely in memory (built-in 5x7 bitmap font, no image or document input),
encodes it with the existing `letratag-protocol` legacy encoder, and sends it
through the existing Engine/journal and BlueZ transport path. No new wire
format; no production rendering change.

Raster: 400 x 32 row-major (feed columns x wire rows).

* Text `RAW TEST`, built-in 5x7 font, at x=20..66, y=3..=9.
* Reference line: exactly **160 feed columns**, y=16, x=20..=179, with
  endpoint ticks y=14..=18 at x=20 and x=179 for unambiguous measurement.
* All ink within printable rows 1..=30.

## Pre-transmission record (before printing)

* Payload: legacy profile, body 1624 bytes, 8 writes
  `[9, 242, 242, 242, 242, 242, 242, 181]` at the validated 244-byte cap.
* ESC D: `1b 44 01 02 90 01 00 00 20 00 00 00` (width 400, height 32,
  alignment 2, legacy mode byte 01).
* SHA-256 raster (packed columns):
  `ea2b68d5d54ed5bfc5a294533d62a07d45051c7fb3488a6ec557e53ae07246a6`
* SHA-256 body:
  `0e2f7648b550333bb7b6cbcdd4c00367f2248fcfe63a98b7f20abe28fc7b340b`
* SHA-256 wire (u16le length-prefixed writes):
  `2720fc133a0d4689cbf909fad9af6db4c540660bfcf53bdb066f8862a13fe057`

Verification before printing:

* In-program decode of the exact capture bytes: PASS (width 400, height 32,
  decoded raster identical to the in-memory source, reference line ink
  exactly at columns 20..=179 = 160 feed columns, neighbouring columns white,
  text present, ink rows 3..=18).
* Independent decoder (`scripts/forensic-raster.py`): PASS (width 400, body
  1624, row 16 ink count exactly 160, ink bbox [20,3,179,18], tick rows
  14..=18 = [2,2,160,2,2]).
* External `sha256sum` of the evidence files matches the in-program hashes.
* `ldd` on the diagnostic binary: no poppler/cups/pappl linkage (libdbus
  present for BlueZ). Components: `letratag-protocol` encoder +
  `letratag-transport` BlueZ + `letratag-printer` Engine/journal only.
* Workspace gates re-run after adding the diagnostic: rustfmt, clippy
  `-D warnings` (all targets/features), 86 default + 105 all-feature tests,
  both IPP suites: all PASS. The example compiles without the `ble` feature
  (stub) so default workspace builds are unaffected.

Evidence files (mode 0600) in `test-output/raw-diag-state/`:
`raw-test.capture.ble`, `raw-test.body.bin`, `raw-test.raster.bin`,
`raw-test-evidence.txt`, `run.log`, plus the archived journal and operator
decision in `archive/`.

## Transmission and operator result

Exactly one physical print, authorized by the operator; printer confirmed on,
lid closed, 12 mm cassette seated, batteries charged. Submitted
2026-10-21T05:13:45Z. No retries; no additional prints.

Diagnostics: connected 1.83 s; Write Request; usable GATT value 495;
validated cap 244; effective write size 244; writes
`[9,242,242,242,242,242,242,181]` (matching the pre-transmission record
exactly); status `1b 52 00` at 9.82 s (7.16 s after the last write); engine
outcome Uncertain at 39.82 s by design. Journal
`1792559637403455342.state` archived with the operator decision.

Operator confirmation: label emerged, `RAW TEST` readable, reference line
visible, orientation correct.

**Operator measurement: reference line 12.9 mm for exactly 160 feed
columns.**

## Interpretation

* Physical pitch: 12.9 mm / 160 columns = 0.080625 mm/column =
  **315.0 dpi apparent feed pitch** (25.4 / 0.080625 = 315.04).
* The 160-dpi layout expectation for 160 columns is 25.4 mm; the line is
  49.2% short.
* This matches the ~315-316 dpi apparent feed pitch measured on the
  2026-10-21 calibration prints (252-dot rectangle = 20.25 mm = 316.1 dpi;
  release-acceptance text 8.4 mm / 104 px = 314.5 dpi) within measurement
  precision.
* **The anomaly reproduces with no PDF, Poppler, PNG, CUPS, IPP, PAPPL or
  document scaling anywhere in the path.** The transmitted payload is
  verified byte-exact (hashes above; independent decode confirms the
  reference line occupies exactly 160 feed columns). The document pipeline
  is therefore ruled out as the cause of the feed-pitch anomaly.
* The historical 2026-10-20 measurements (~159 dpi feed, three consistent
  prints, isotropic ~160 dpi) remain in conflict with the current
  ~315 dpi apparent feed pitch (now six consistent prints across CUPS,
  Direct IPP and RAW paths, battery-independent). The trigger (device state,
  feed-stepping mode, or an unidentified mechanism) remains unresolved.
  No mechanism is assumed and no DPI compensation was applied.

## Consequences

* The v0.3.0 NO-GO for dimensional-accuracy claims stands; this diagnostic
  strengthens it by eliminating the entire document pipeline as a suspect.
* The remaining suspects are the printer's feed stepping / firmware state
  and the host-side BLE write behavior. The wire bytes are verified correct,
  and write lengths/timing match every prior successful print, so a
  transport corruption mechanism is not supported by any retained evidence.
* Any future fix must be evidence-driven: raw payload capture during a
  state transition, or vendor feed-pitch documentation. Do not change the
  layout DPI or add feed compensation from this result alone.

Hygiene note: some older hardware journals outside archived directories
remain on disk (e.g. `test-output/hardware-12/...state`, inflight); their
outcomes are recorded in the dated evidence documents. They belong to other
state directories and did not block this diagnostic.
