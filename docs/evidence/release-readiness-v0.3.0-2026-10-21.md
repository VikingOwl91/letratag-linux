# Release readiness report: v0.3.0 acceptance, 2026-10-21

## Verdict

**NO-GO for v0.3.0 as a release that claims dimensionally accurate
printing.** All Phase 1 software gates pass, the Direct IPP workflow is
verified end to end against mock and physically for readability, and both
authorized physical labels were operator-confirmed readable. The physical
calibration label, however, shows a significant, reproducible, unexplained
dimensional mismatch along the feed direction: the transmitted 252-dot
rectangle (40.0 mm at the 160-dpi software layout) measured **20.25 mm**
(49.4% short, ~316 dpi apparent feed pitch). Per the release criteria, a
significant unexplained dimensional mismatch is a release blocker for
dimensional-accuracy claims. No DPI, protocol, rendering, or BLE change was
made to compensate. No tag, crate publication, or release was created.

A GO is only defensible if the release explicitly makes **no**
dimensional-accuracy claim and documents the current feed anomaly as a known
limitation; even then, labels printed in the current device state are about
half the intended feed length, which is likely unacceptable for a label
printer release. The recommended path is to resolve the feed-stepping
mechanism first (see Remaining risks).

## Phase 1: software gates (all PASS)

Executed locally with the pinned staged Rust/Cargo 1.98.1, offline vendored
dependencies, and the pinned patched PAPPL 1.4.9 prefix. No network, no
hardware, no system queue changes for these gates.

| Gate | Result |
|---|---|
| `cargo fmt --all -- --check` | PASS |
| `rustfmt --check tests/make-pdf.rs tests/inspect-direct-captures.rs` | PASS |
| `cargo test --locked --workspace` | PASS: 86 passed, 0 failed |
| `cargo test --locked --workspace --all-features` | PASS: 105 passed, 0 failed |
| `cargo clippy --locked --workspace --all-targets --all-features -- -D warnings` | PASS |
| `bash scripts/test-direct-ipp.sh` | PASS: explicit 50/63.5-mm media, lp/lpr/ipptool, state queries, mismatch rejection, 4 independently decoded legacy mock captures (3 x 315x32, 1 x 400x32, identical artwork x44..266/y11..19, +85 blank columns), 3 geometry-oracle unit tests, inflight-poll fixture |
| `bash scripts/test-ipp.sh` | PASS: PDF-only receive policy, invalid-input rejection, direct lp/lpr, 4 mock captures, bounded job retention, multipage byte preservation |
| `python3 scripts/test-forensic-raster.py` | PASS: 5 independent wire-decoder tests |
| `bash scripts/test-cups-geometry.sh` | PASS: offline CUPS shrink reproduction, geometry guard, scaling-none control (no queue, no hardware) |
| Documentation link check | PASS: 87 relative links in 14 user-facing files, 0 failures |
| CUPS Experimental labeling | PASS: present in README, CHANGELOG, both new guides, ADR, PDF, security, protocol, FFI, packaging, hardware and verification docs |
| Direct IPP recommended/default labeling | PASS: README, direct-ipp.md, hardware-acceptance.md, verification.md, ADR, pdf-integration.md, packaging, CHANGELOG |
| Direct IPP without system scheduler/queue | PASS: `lp -h 127.0.0.1:38692 -d letratag ...` completed as PAPPL job `letratag-1` while the system scheduler job list was unchanged; no queue registration involved |
| Shell syntax (`bash -n`) | PASS for both new scripts |

Release-readiness gaps found and dispositioned with the operator:

* Working tree was dirty (workflow docs, tests, evidence). Committed locally
  as `eb3cc52`; not pushed, not tagged.
* Stale `letratag-e2e` queue removed with operator approval
  (`lpadmin -x letratag-e2e`); `Canon_LBP633C` untouched.
* Outstanding job-48 inflight journal archived with the operator decision
  before any new physical print.
* No v0.3.0 release notes exist yet; workspace version remains 0.1.1
  (libraries published at 0.1.1; v0.2.0 tag predates that bump). Version
  bump and release notes are premature while the recommendation is NO-GO.
* `packaging/arch/PKGBUILD.in` remains a template with `@...@` placeholders;
  not release-ready by design.

## Phase 2: physical hardware acceptance (readability PASS, dimensional accuracy FAIL)

Operator confirmed before printing: printer on, lid closed, 12 mm cassette
seated, batteries charged. Exactly two prints were authorized and executed;
no retries, no CUPS, no additional prints. The printer advertised on BLE;
a read-only connect/disconnect preflight succeeded.

Endpoint: `letratag-printer serve --state test-output/release-state --spool
test-output/release-spool --port 8631 --transport bluez --address
90:E5:B1:49:DA:AE --profile legacy --chunk-mode auto --diagnostics`, with the
validated 244-byte cap seeded in `printer-caps.conf`.

### Preflight payload evidence (captured before transmission, mock transport, identical document/profile/cap)

| Job | Document | Frame | ESC D | Writes @244 | Raster SHA-256 | Body SHA-256 | Wire SHA-256 |
|---|---|---|---|---|---|---|---|
| Print 1 | asset-0001.pdf (50 x 12 mm) | 315 x 32 | `1b 44 01 02 3b 01 00 00 20 00 00 00` | `[9,242,242,242,242,242,82]` | `9f408b2b...7a6af` | `83bb8fbd...5999` | `5504251d...89e8` |
| Print 2 | calibration-160.pdf (50 x 12 mm) | 315 x 32 | `1b 44 01 02 3b 01 00 00 20 00 00 00` | `[9,242,242,242,242,242,82]` | `8bc8c671...999d` | `0d293d0c...1394` | `0c1fca03...cf11b` |

Full hashes are recorded in the journal decision files. The print-2 raster
hash matches the independently verified direct-315 fixture, and the artwork
(rectangle 252-dot edge, 30 occupied rows, ticks every 63 dots) matches the
source PDF at the 160-dpi layout. Rendering and packing are correct.

Note: the engine writes `.ble` captures only on a certain outcome; hardware
jobs end Uncertain by design, so the actual hardware bytes are not captured
by the engine. The write sequences below corroborate the preflight payload
(lengths and count match exactly); individual hardware payload hashes remain
unavailable, as documented in the forensic report.

### Print 1: normal 50 x 12 mm label (Direct IPP)

Command: `lp -h 127.0.0.1:8631 -d letratag -o media=custom_default_12x50mm
-o print-scaling=none test-output/acceptance/asset-0001.pdf`, submitted
2026-10-21T04:48:40Z.

Diagnostics: connected 1.83 s; Write Request; usable GATT value 495;
validated cap 244; effective write size 244; writes `[9,242,242,242,242,242,82]`
(final 2.54 s); status `1b 52 00` at 8.73 s (6.19 s after last write);
outcome Uncertain at 38.73 s (by design). Journal archived with the operator
decision.

Operator confirmation: label emerged, `ASSET-0001` / `DYMO LT200B`
readable, orientation correct. Measurements: total cut-to-cut 43 mm,
lead-in 11 mm, text 8.4 mm (feed) x 4.25 mm (across). Derived apparent
pitch from the text: 104 px / 8.4 mm = 314.5 dpi feed; 26 rows / 4.25 mm =
155.4 dpi across.

### Print 2: calibration label (Direct IPP, explicit media-col 1200 x 5000)

Command: `ipptool -d length=5000 -f test-output/acceptance/calibration-160.pdf
ipp://127.0.0.1:8631/ipp/print examples/print-pdf.test` (explicit
media-col 1200 x 5000, zero margins, print-scaling none, one copy),
submitted 2026-10-21T04:53:01Z.

Diagnostics: connected 1.78 s; identical transport parameters; writes
`[9,242,242,242,242,242,82]` (final 2.49 s); status `1b 52 00` at 8.64 s
(6.15 s after last write); outcome Uncertain at 38.64 s (by design). Journal
archived with the operator decision.

Operator confirmation: label emerged, landmarks visible, orientation
correct. Measurements: rectangle 20.25 mm (feed) x 4.75 mm (across),
lead-in 10 mm, lead-out 13.25 mm, total 43.50 mm.

## Phase 3: comparison against expected dimensions

Expected values derive from the unchanged 160-dpi software layout and the
verified transmitted raster (not from physical assumptions). No formal
numeric tolerance is documented in the project; the deviations below are far
outside any plausible measurement tolerance.

| Quantity | Transmitted | Expected (160-dpi layout) | Measured | Apparent pitch | Deviation |
|---|---|---|---|---|---|
| Rectangle width (feed) | 252 dots | 40.00 mm | 20.25 mm | 316.1 dpi | -19.75 mm (-49.4%) |
| Tick interval (feed) | 63 dots | 10.00 mm | 5.06 mm (derived: 20.25/4) | 316.1 dpi | -49.4% |
| Rectangle height (across) | 30 rows | 4.76 mm | 4.75 mm | 160.4 dpi | -0.01 mm (-0.3%) |
| Total cut length | 315 cols | ~53.4 mm (50.0 + ~3.4 lead-in) | 43.50 mm | — | -9.90 mm (-18.5%) |
| Print 1 text (feed) | 104 px | 16.5 mm | 8.4 mm | 314.5 dpi | -49.1% |
| Print 1 text height (across) | 26 rows | 4.13 mm | 4.25 mm | 155.4 dpi | +2.9% |

Findings:

1. **Rendering correctness: PASS.** The transmitted raster is exactly the
   intended artwork (hashes above; independent decoder; artwork oracle).
   Across-tape pitch is ~160 dpi as documented. The write segmentation
   matches the preflight payload exactly on both prints.
2. **Physical dimensional accuracy: FAIL along feed.** The rectangle is
   49.4% short along feed. This reproduces jobs 44/46/47 exactly (20.25 mm
   rectangle, ~43.2 mm total, 10 mm lead-in) and is now reproduced through
   the **Direct IPP** path, ruling out CUPS as the cause of this anomaly.
   The separate CUPS `pdftopdf` fitting defect (0.787402 two-axis shrink)
   remains documented and is not involved in these prints.
3. **Mechanism: unresolved.** 2026-10-20 prints measured ~159 dpi feed
   (three consistent prints, isotropic ~160 dpi); 2026-10-21 prints measure
   ~305-316 dpi feed (now five consistent prints, battery-independent,
   path-independent). The trigger (feed-count threshold, persistent device
   state, or measurement-era difference) is unidentified. The total length
   does not reconcile with a uniform-pitch model in either era, so even the
   lead-in/trailing behavior is not fully characterized.
4. **No compensation applied.** Per the release criteria, no DPI change,
   anisotropic model, or feed correction was introduced. Changing the layout
   DPI to ~316 would break the 2026-10-20 state and the across pitch, and
   would be an unverified workaround.

## Remaining risks and uncertainties

* The feed-stepping trigger is unknown; the device may change state again.
  Any fixed DPI choice is unsafe in at least one observed state.
* Actual hardware payload bytes are not captured (engine writes captures only
  on certain outcomes); hardware evidence is write lengths/timing plus the
  deterministic preflight payload.
* The 2026-10-20 ~160-dpi measurements were ruler-based with a corrected
  caliper misread; the 2026-10-21 measurements are caliper-based. A
  measurement-era contribution cannot be fully excluded, though five
  consistent prints today make a pure measurement artifact unlikely.
* Lead-in/trailing behavior is not modeled and varies between eras (3.4 mm
  vs 10-11 mm observed).
* Desktop application compatibility for Direct IPP is unverified.
* CUPS custom-media handling remains Experimental with no fully verified
  custom-length workaround.
* The systemd template, packaged installation, and non-root service-user
  D-Bus permissions remain untested.

## Recommendation

**NO-GO for v0.3.0 with any dimensional-accuracy claim.** Software gates
pass and the Direct IPP workflow is verified, but the physical calibration
fails the dimensional criterion by ~2x along feed, reproducibly and
independently of CUPS. Before a GO:

1. Identify the feed-stepping trigger (controlled same-width prints across
   device state changes, with raw payload capture enabled for certain
   outcomes or an external BLE sniffer), or
2. Obtain vendor/firmware documentation for the feed pitch, and
3. Re-run this acceptance after the mechanism is understood, with the
   layout either confirmed at the true native pitch or explicitly
   documented as device-state-dependent.

A release without dimensional-accuracy claims is possible only with prominent
known-limitation notes; that is a product decision for the operator, not a
test outcome.
