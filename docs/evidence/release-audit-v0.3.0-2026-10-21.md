# v0.3.0 release audit, 2026-10-21

Independent audit of the LetraTag Linux project for the v0.3.0 release
decision. Software validation, hardware evidence and unverified claims are
kept apart. No tag, no publication, no version bump and no physical print
were performed by this audit.

## 1. Commit under review

* Starting point: `596a380` "docs: record hardware validation confirming
  dimensional accuracy".
* The audit was performed on the commits listed in section 4, ending with
  the release-candidate commit on the default branch.
* Working tree at the end of the audit: clean.

## 2. Findings

| # | Severity | Finding | Status |
|---|---|---|---|
| 1 | Release blocker | The standard IPP attribute `printer-resolution` advertised the logical layout resolution (200 dpi) instead of the resolution at which the printer prints (RFC 8011). A custom attribute carrying the physical value does not make the standard attribute semantically valid. | Fixed, commit `9df2588`, regression assertions in `tests/ipp/get-printer.test` |
| 2 | Release blocker | The CI gate `rustfmt --check tests/make-pdf.rs tests/inspect-direct-captures.rs` failed: `tests/inspect-direct-captures.rs` was not formatted. CI would have failed on the release commit. | Fixed, commit `ffff17a` |
| 3 | Major (documentation) | The living documentation still described the pre-calibration model: 160 dpi layout, an unresolved 160-vs-316-dpi feed conflict, "awaits hardware validation", and the old `letratag-physical-resolution` attribute. | Fixed, commit `8e08995` |
| 4 | Major (claims) | The dimensional-accuracy wording mixed calculated model residuals (<= 0.013 mm) with measurement uncertainty. The readings were reported to the nearest millimetre (about +/-0.5 mm), not to the caliper's 0.05 mm resolution, so the old phrasing implied an accuracy the measurements cannot support. | Fixed, commit `8e08995` (evidence document, verification record, changelog) |
| 5 | Minor (test quality) | `crates/letratag-printer/src/engine.rs` contained a vacuous assertion (`assert!(... || true)`) in the btleplug connect-failure test. | Fixed, commit `ffff17a`, replaced with the real lock-file contract |
| 6 | Minor (release hygiene) | A Python bytecode cache (`scripts/__pycache__/forensic-raster.cpython-314.pyc`) was tracked in git; one document referenced an absolute local path of the reference project. | Fixed, commit `0dacd63` |
| 7 | Minor (coverage) | No automated check existed for documentation links, although the verification record claimed "documentation links pass". | Added `scripts/check-docs.py` and a CI step, commit `ffff17a` |
| 8 | Minor (crate metadata) | The published `letratag-transport` 0.1.1 predates the btleplug backend, so the crate content no longer matched the published version; its description and README mentioned BlueZ only. | Fixed, commit `33d4f83`: version 0.1.2 prepared (not published), description, README with a compiled btleplug example, keywords and categories, license text copies with a CI drift check |

No other release-blocking defect was found. No speculative architectural
change was made; the protocol, the BLE transports, the mock transport, the
error semantics and the rendering pipeline were left untouched.

## 3. What was verified independently

* **Fixture integrity.** `crates/letratag-transport/tests/fixtures/*` are
  byte-identical to the recorded hashes (`hello-linux-legacy.ble`
  `357a6b8d...eeb98b`, `hello-linux.png` `9722e3df...cc1f6e8`,
  `raw-raster-diagnostic.ble` `2720fc13...fe057`).
* **Geometry model.** Millimeter conversions go through named functions with
  round-half-away-from-zero rounding; logical pixels, physical pixels and
  millimeters are never mixed silently (`crates/letratag-printer/src/geometry.rs`).
* **Calibration traceability.** The named profile `LT200B_2026_10_21`
  (315 DPI feed, 160 DPI cross-tape) is documented in `docs/calibration.md`
  and traceable to the dated evidence document
  `docs/evidence/hardware-validation-2026-10-21.md`.
* **IPP advertisement.** Verified on the wire, not only through the client:
  the raw IPP response carries `printer-resolution-supported =
  315x160dpi`, `printer-resolution-default = 315x160dpi` and
  `letratag-logical-resolution = 200dpi`, each exactly once, in the
  printer-attributes group. The same values were confirmed independently by
  an `ipptool` JSON dump and by a libcups `ippFindAttribute` probe.
* **Network exposure.** The PAPPL system binds `127.0.0.1` only
  (`crates/letratag-printer/native/adapter.c`), with TLS disabled by design
  for a loopback-only service.
* **Input limits.** PDF input is capped at 8 MiB by the PAPPL document
  policy and re-checked in Rust; Poppler and qpdf run under `prlimit` in a
  private temporary directory that is removed when the job ends.
* **BLE selection.** The transport requires exactly one matching GATT
  service and the expected characteristic set, otherwise it fails with a
  profile error before any write.
* **No automatic replay.** A hardware job writes an `inflight` journal entry
  before the first possible write; every failure after that point is
  reported as `Uncertain` and is never retried automatically.
* **No unintended printing.** Mock is the default transport, the test
  suites are mock-only, and the one example that can transmit requires
  `--acknowledge-unverified-hardware`.

## 4. Changes made by this audit

| Commit | Subject |
|---|---|
| `9df2588` | fix(ipp): advertise the physical raster resolution in printer-resolution |
| `8e08995` | docs: refresh the living docs for the calibrated model and the IPP advertisement |
| `ffff17a` | test: fix the release gates CI would fail on |
| `0dacd63` | chore: keep release artifacts clean |
| `c003eb1` | docs: prepare the v0.3.0 release candidate |
| `33d4f83` | chore(crates): prepare letratag-transport 0.1.2 for publication |

## 5. Gates run

All gates ran offline against the mock transport.

| Gate | Command | Result |
|---|---|---|
| Formatting | `cargo fmt --all -- --check`, `rustfmt --check tests/*.rs` | clean |
| Lint | `cargo clippy --locked --workspace --all-targets --all-features -- -D warnings` | clean |
| Default tests | `cargo test --locked --workspace` | 113 passed, 0 failed |
| All-feature tests | `cargo test --locked --workspace --all-features` | 147 passed, 0 failed |
| PDF-only IPP suite | `bash scripts/test-ipp.sh` | passed |
| Direct IPP suite | `bash scripts/test-direct-ipp.sh` | passed |
| Forensic decoder | `python3 scripts/test-forensic-raster.py` | 5 tests, passed |
| CUPS geometry control | `bash scripts/test-cups-geometry.sh` | passed |
| Calibration diagnostic | `examples/calibration-label.rs --check` | passed (1364 columns, 5 reference lines) |
| Documentation links | `python3 scripts/check-docs.py` | passed |
| License inventory | `bash scripts/check-licenses.sh` | passed |

An earlier run of the same suite failed intermittently; the cause was a full
`/tmp` tmpfs in the shared environment, not the code. With `TMPDIR` on the
main filesystem the suite passes consistently (3 of 3 runs).

## 6. Hardware evidence evaluated

* One operator-authorized Direct IPP calibration label, 2026-10-21
  (`docs/evidence/hardware-validation-2026-10-21.md`): reference ink lengths
  from 10 mm to 100 mm reported as 10/20/40/50/100 mm against commanded
  9.9987/19.9975/39.9949/49.9937/99.9873 mm; all five imply 314.96 DPI feed;
  band height 4.75 mm against a 4.7625 mm model; lead-in 19 mm, lead-out
  about 9 mm, total about 127 mm.
* One operator-authorized btleplug HELLO LINUX print: readable, correctly
  oriented, 12.3 mm text width, identical to the BlueZ backend
  (`docs/evidence/btleplug-hardware-acceptance-2026-10-21.md`).
* Connect-only backend comparison (`docs/evidence/backend-comparison-2026-10-21.md`).

## 7. Supported claims and their scope

Supported:

* Native Linux software support for the DYMO LetraTag 200B, with Direct IPP
  as the recommended workflow.
* The calibrated Direct IPP PDF printing path has been physically validated
  for reference ink lengths from 10 mm to 100 mm, with results consistent
  with the raster model within the precision of the measurements performed.
* Mock transport with deterministic, hardware-independent tests.
* Optional btleplug backend alongside the default BlueZ backend.

Not supported, and not claimed:

* Universal dimensional accuracy across devices, media types or input paths.
* Any accuracy claim below the resolution of the measurements performed.
* Support for operating systems other than Linux, or for DYMO models other
  than the LetraTag 200B.
* A production-ready CUPS queue (still Experimental, with a demonstrated
  `pdftopdf` media-fitting defect).
* Automatic confirmation that a physical label was printed.

## 8. Remaining limitations

1. The terminal-status ambiguity is unresolved: the printer reports success
   even with the lid open, so every hardware job stays uncertain and
   operator-in-the-loop.
2. The calibration is empirical and validated on one device with one label,
   on the feed axis only.
3. CUPS queue integration still needs resolved forwarded media-col and
   post-filter geometry checks.
4. The systemd template is not live-tested; the packaging template needs a
   public project URL and a reviewed source archive.
5. The btleplug backend is hardware-accepted, but BlueZ remains the default
   and the recommended real-hardware backend.

## 9. Packaging and installation status

* Installation from source is reproducible with the documented steps; the
  release notes list the system prerequisites, the private PAPPL build and
  the Direct IPP quick start.
* `packaging/arch/PKGBUILD.in` and `packaging/systemd/letratag-linux.service`
  are preparation templates and are not installed or activated by the
  project.
* The three library crates are published at 0.1.1; the application crate is
  deliberately unpublished. The workspace version is unchanged in v0.3.0,
  so no library republication is required.

## 10. Release artifact readiness

* Source tree at the release tag: ready.
* Release notes draft: `docs/releases/v0.3.0.md`, ready for review.
* Changelog: restructured into a `v0.3.0` release-candidate section.
* Binary package: not produced; the PKGBUILD template needs a public URL
  and a real source checksum.

## 11. Recommendation

**CONDITIONAL GO for v0.3.0.**

The software is validated, the advertisement is now semantically correct,
the release gates pass, and the user-facing claims match the hardware
evidence. The condition is procedural: the operator reviews the release
notes and the packaging template, then performs the tagging and publication
steps explicitly.

No publication, tag, push or physical print is authorized by this audit.

### Proposed release sequence (operator-controlled)

1. Review `docs/releases/v0.3.0.md` and this audit.
2. Confirm the release commit on the default branch.
3. `git tag -a v0.3.0 -m "v0.3.0"`, then push the tag.
4. Build the release artifacts from the tagged commit; fill the PKGBUILD
   template with the public URL and the source archive SHA256.
5. Publish the library crates only if a new library version is intended
   (not required for v0.3.0).
6. Announce the release with the reviewed notes.
