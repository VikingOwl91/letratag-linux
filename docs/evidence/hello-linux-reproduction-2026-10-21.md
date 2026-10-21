# HELLO LINUX historical reproduction, 2026-10-21

Reproduce the first successful `HELLO LINUX` hardware print of 2026-10-20
(attempt 7) as faithfully as possible, to determine whether the original
print path still produces the historically observed ~160 dpi apparent feed
pitch or the ~315-316 dpi apparent feed pitch measured today.

## Phase 1: historical reconstruction

Original artifacts (all byte-identical across locations):

* `test-output/hello-linux.png` (1000 x 80 px, 2326 bytes, SHA-256
  `9722e3dfd711d795d64f338577921287582d9cacd054c2b0407e0efc1cc1f6e8`),
  identical in `/tmp/opencode/hello-linux.png` and
  `docs/evidence/geometry-2026-10-21/hello-linux.png`. The original PNG
  generator script was not retained; the PNG itself is the original input.
* Retained Oct 20 mock capture `test-output/hello-mock/1792514771648929798.ble`
  (SHA-256 `6aae620dfb53a37debc7d0a98502fc949dd6ed8f0d44473d4253a3ba89c5a9ee`).
* Attempt 7 evidence: `docs/evidence/hardware-attempt-7-2026-10-20-success.md`,
  diagnostics `test-output/hardware-7-diag.log`, journal decision
  `test-output/hardware-7/archive/DECISION.md`.

Original implementation: `hardware-send` with `--columns 400 --profile legacy
--max-write 244 --linger-seconds 20`. Rendering: `letratag-render`
`render_png` with width=400, active_rows=30, threshold=128, dither=false.
Code currency: since 2026-10-19 the render and protocol crates have only
docs/README commits; the transport changes (chunk sizing, discovery
resolution) do not alter the payload. The render logic that produced the
Oct 20 raster is unchanged.

Reconstruction confidence: **high.** The current code reproduces the retained
Oct 20 mock capture **byte-for-byte** (same SHA-256), proving the
PNG -> raster -> modern-profile payload chain is bit-identical to the
original.

## Phase 2: preflight (no print)

Reconstructed legacy payload (attempt 7 configuration), generated without
PDF, Poppler, CUPS, IPP or PAPPL:

* Raster: 400 x 32; ink bbox x124..=275, y8..=23 — **152 x 16 occupied
  pixels** (center span 151 x 15). Matches the historical record (152 ink
  columns, 16 rows, 24.3 mm measured 2026-10-20). Raster SHA-256
  `76365e366e47c7aa0f9bbfe49c905125c7306c3a8ed9567da6c39cfba9681927` —
  identical to the raster inside the retained Oct 20 mock capture.
* Commands: `ESC s 9a 02 00 00` / `ESC D 01 02` width=400 height=32 /
  `ESC E` / `ESC A` / `ESC Q`.
* Body 1624 bytes; 8 BLE writes `[9, 242, 242, 242, 242, 242, 242, 181]` at
  the validated 244-byte cap — matches the attempt 7 diagnostic trace exactly
  (`[9,242 x6,181]`).
* Body SHA-256 `58272474123e691ac5d206d11cc91c677efc1b5119a346bb1c7451514d069c53`;
  wire SHA-256 (u16le length-prefixed writes)
  `357a6b8dad5a11ca68a39b9825cee3d91551c195f69d7065e07cef0df0eeb98b`.
* Independent decode (`scripts/forensic-raster.py`): PASS (width 400, body
  1624, ink bbox [124,8,275,23], occupied 152 x 16). PBM visual retained.
* External `sha256sum` matches the in-program hashes.

Expected physical text width from the verified 152 occupied columns:

| Model | Expected width (occupied) | Expected width (center span 151) |
|---|---|---|
| 160 dpi (0.15875 mm/col) | 24.13 mm | 23.97 mm |
| 315 dpi (0.080625 mm/col, RAW-measured) | 12.26 mm | 12.18 mm |

Comparison with today's RAW raster diagnostic: same frame width 400, legacy
profile, cap 244, write sequence, ESC D height/alignment/mode and command
suffix; different raster content (PNG-rendered text, 152 ink columns, vs the
in-memory 160-column reference line).

## Phase 3: physical reproduction

Operator confirmed readiness (on, lid closed, cassette seated, batteries
charged). Exactly one print, 2026-10-21T05:20:58Z, using the original
command. (An earlier invocation at 05:20:44Z failed before any BLE activity
because the debug binary had been rebuilt without the `ble` feature by the
IPP test scripts; it created no journal and transmitted nothing.)

Diagnostics: connected 1.88 s; Write Request; usable GATT value 495;
validated cap 244; effective write size 244; writes
`[9,242,242,242,242,242,242,181]`; status `1b 52 00` at 9.87 s (7.16 s after
the last write; attempt 7: 7.24 s); outcome Uncertain at 39.87 s by design.
No retries. Journal `1792560060268561654.state` archived with the operator
decision. Evidence: `test-output/hello-repro-state/` (PREFLIGHT.md, payload
files, decoded PBM, run.log, archive/DECISION.md).

Operator confirmation: label emerged, `HELLO LINUX` readable, orientation
correct. **Operator measurements: text width 12.3 mm (feed), text height
about 2.25 mm (across).**

## Phase 4: interpretation

| Quantity | 2026-10-20 (attempt 7/A/B) | 2026-10-21 reproduction | Apparent pitch today |
|---|---|---|---|
| Text width (feed), 152 cols | 24.3 mm (ruler) | **12.3 mm** (operator) | 313.9 dpi (occupied); 311.8 dpi (center span) |
| Text height (across), 16 rows | 2.25-2.5 mm | **2.25 mm** | ~169-181 dpi (coarse); consistent with prior ~160-180 |
| Ratio today/historical | 1.00 | 0.506 | ~2x compression along feed |

Cross-checks against today's other measurements: RAW reference line 160 cols
= 12.9 mm = 315.0 dpi (0.080625 mm/col); HELLO text 152 cols = 12.3 mm =
0.080921 mm/col. Per-column pitches agree within 0.4%. The calibration
rectangle (252 cols = 20.25 mm) gives 316.1 dpi. Today's device state
consistently steps about 0.0806-0.0809 mm per feed column.

Conclusions:

1. **Not an implementation difference.** The reconstructed payload is
   byte-faithful to the original (retained raster and mock capture
   reproduced bit-identically; write sequence identical to the attempt 7
   trace), yet it now prints at half the historical feed width. The same
   code and payload produce a different physical result today.
2. **Not a path difference.** The ~315 dpi apparent feed pitch now appears
   across the original PNG/hardware-send path, the Direct IPP path, the
   CUPS path, and the document-free RAW raster diagnostic.
3. **Device state or physical feed behavior changed between the eras.**
   Battery was ruled out (fresh-battery print reproduced the anomaly), and
   the across pitch is unchanged (2.25 mm both eras). The trigger remains
   unidentified; no mechanism is claimed.
4. **Historical measurement discrepancy is unlikely but not excluded.** The
   Oct 20 feed measurements were ruler-based (with one corrected caliper
   misread), while today's are caliper-based. However, each era is
   internally consistent across 3 (Oct 20) and 6+ (Oct 21) prints, the pixel
   counts are verified from retained rasters in both eras, and the across
   measurements agree between eras — so a pure measurement artifact would
   have to affect only the feed axis only in one era. The evidence
   favors a real change in physical feed behavior.
5. **No compensation applied.** No DPI, protocol or BLE change was made.
   The v0.3.0 NO-GO for dimensional-accuracy claims stands.
