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
| 9 | Minor (build hygiene) | The documented plain build (`cargo build --locked --workspace`) emitted a dead-code warning for the chunk-mode resolver, which only the `pappl` and `ble` commands use. CI did not catch it because its clippy run uses all features. | Fixed, commit `a9670fe`; every documented feature combination now builds without warnings |
| 10 | Release blocker (found in the packaging preflight) | The library test suite failed intermittently, about one run in ten, with "queue already owned by another process". The queue lock is an `flock` on the journal's `queue.lock` descriptor. A child process that inherited the descriptor during `fork(2)` keeps the open file description alive, so the lock survived the engine that took it. The document tests spawn helper processes, which is what made the race visible. The same hazard exists in production, where the server spawns Poppler and qpdf while holding the journal lock. | Fixed, commit `14d5eb3`: the engine releases the lock explicitly when it is dropped, with the regression test `queue_lock_is_released_when_the_engine_is_dropped`. Measured: 0 failures in 30 consecutive suite runs, against 4 in 30 before the fix |
| 11 | Major (packaging) | The Arch recipe was a template with placeholders (`@PROJECT_URL@`, `@PATCHED_PAPPL_PACKAGE@`, `@SOURCE_SHA256@`) and a stale version (0.1.0). It could not be built. | Fixed: `packaging/arch/PKGBUILD` is a complete recipe with a real source URL, a real checksum of the actual release archive, an accurate dependency list and a self-contained patched-PAPPL build. Verified locally with `makepkg --verifysource` and a full build |
| 12 | Minor (version metadata) | The installed binary reported `letratag-printer 0.1.1` while the release and the Arch package are 0.3.0. | Fixed, commit `81cbbc5`: the application crate carries the release version; the library crates keep the workspace version that is published |
| 13 | Minor (release process) | There was no reproducible way to produce the release source archive, so the packaging checksum had no defined basis. | Fixed: `scripts/make-release-archive.sh` produces a content-only archive (no commit id, no timestamps, `packaging/arch/` excluded), so its checksum is stable and the recipe can pin it |

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

### Packaging and publication verification (packaging preflight)

* **Arch package.** `makepkg --verifysource` passes for both sources
  (the release archive and the upstream PAPPL 1.4.9 tarball). A full
  `makepkg` run built `letratag-linux-0.3.0-1-x86_64.pkg.tar.zst` on Arch
  Linux; the recipe's own `check()` phase ran the complete test suite. The
  package installs the binary, the mock-only systemd unit, the project
  license and the PAPPL license and notice, and nothing else.
* **Runtime dependencies.** The packaged binary links `libcups`, `libavahi`,
  `libcrypto`, `libjpeg`, `libpng` and `libdbus-1` dynamically and PAPPL
  statically, so the recipe's dependency list matches the binary exactly.
  The helper programs it runs (`pdftoppm`, `qpdf`, `prlimit`) are covered as
  well.
* **crates.io packaging.** `cargo package -p letratag-transport` assembled
  13 files and verified them by building the package. The normalized
  manifest contains no path dependency: the only workspace crates are
  dev-dependencies on `letratag-protocol` 0.1.1 and `letratag-render` 0.1.1,
  both already published. All four feature combinations build.
* **Release archive.** The archive is content-only: two consecutive runs and
  two different commits with the same tree produce byte-identical archives.
* **Not verified.** `namcap` is not installed on the maintainer machine and
  needs root to install, so no `namcap` report exists. The package was built
  without a chroot (`makepkg` as an unprivileged user), so a distribution
  build with the full clean chroot was not exercised. The package was not
  installed with `pacman` and no systemd unit was activated.

## 4. Changes made by this audit

| Commit | Subject |
|---|---|
| `9df2588` | fix(ipp): advertise the physical raster resolution in printer-resolution |
| `8e08995` | docs: refresh the living docs for the calibrated model and the IPP advertisement |
| `ffff17a` | test: fix the release gates CI would fail on |
| `0dacd63` | chore: keep release artifacts clean |
| `c003eb1` | docs: prepare the v0.3.0 release candidate |
| `33d4f83` | chore(crates): prepare letratag-transport 0.1.2 for publication |
| `a9670fe` | fix(build): keep the default build warning-free |
| `14d5eb3` | fix(engine): release the queue lock explicitly when the engine is dropped |
| `5ba5f55` | packaging: refresh the release archive checksum after the engine fix |
| `81cbbc5` | chore(release): align the application version and document the artifacts |

## 5. Gates run

All gates ran offline against the mock transport.

| Gate | Command | Result |
|---|---|---|
| Formatting | `cargo fmt --all -- --check`, `rustfmt --check tests/*.rs` | clean |
| Lint | `cargo clippy --locked --workspace --all-targets --all-features -- -D warnings` | clean |
| Default tests | `cargo test --locked --workspace` | 114 passed, 0 failed |
| All-feature tests | `cargo test --locked --workspace --all-features` | 148 passed, 0 failed |
| Library suite, repeated | `cargo test --locked -p letratag-printer --all-features --lib` | 30 consecutive runs, 0 failures (4 in 30 before the queue-lock fix) |
| PDF-only IPP suite | `bash scripts/test-ipp.sh` | passed |
| Direct IPP suite | `bash scripts/test-direct-ipp.sh` | passed |
| Forensic decoder | `python3 scripts/test-forensic-raster.py` | 5 tests, passed |
| CUPS geometry control | `bash scripts/test-cups-geometry.sh` | passed |
| Calibration diagnostic | `examples/calibration-label.rs --check` | passed (1364 columns, 5 reference lines) |
| Documentation links | `python3 scripts/check-docs.py` | passed |
| License inventory | `bash scripts/check-licenses.sh` | passed |
| Package sources | `makepkg --verifysource` | passed for both sources |
| Package build | `makepkg` (Arch Linux) | built `letratag-linux-0.3.0-1-x86_64.pkg.tar.zst`, recipe `check()` phase passed |
| Crate packaging | `cargo package -p letratag-transport` | 13 files, verification build passed |

An earlier run of the same suite failed intermittently; one cause was a full
`/tmp` tmpfs in the shared environment, which is not a code defect. The
other cause was the queue-lock race described in finding 10, which is a
real defect and is fixed. With `TMPDIR` on the main filesystem the suite
passes consistently.

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
4. The systemd template is not live-tested; the packaged service was never
   activated.
5. The btleplug backend is hardware-accepted, but BlueZ remains the default
   and the recommended real-hardware backend.
6. The Arch package was built and inspected but not installed with `pacman`,
   and no `namcap` report exists because `namcap` needs root to install.

## 9. Packaging and installation status

* Installation from source is reproducible with the documented steps; the
  release notes list the system prerequisites, the private PAPPL build and
  the Direct IPP quick start.
* `packaging/arch/PKGBUILD` is a complete recipe that builds on Arch Linux
  (`makepkg --verifysource` and a full `makepkg` run both pass). It is not
  an AUR submission and installs nothing by itself.
* `packaging/systemd/letratag-linux.service` stays a mock-only template and
  is never activated by the project.
* The three library crates are published at 0.1.1;
  `letratag-transport` 0.1.2 is prepared and verified but not published. The
  application crate is deliberately unpublished and now carries the release
  version 0.3.0.

## 10. Release artifact readiness

* Release archive: `scripts/make-release-archive.sh 0.3.0` produces a
  reproducible, content-only `letratag-linux-0.3.0.tar`; the Arch recipe
  pins its checksum.
* Release notes draft: `docs/releases/v0.3.0.md`, ready for review, with
  the artifact list, the publication order and the recovery procedure.
* Changelog: restructured into a `v0.3.0` release-candidate section.
* Binary package: `letratag-linux-0.3.0-1-x86_64.pkg.tar.zst` built and
  verified locally; publishing it needs the release asset to exist.
* Prebuilt release binary: buildable with the documented command; not
  published.

## 11. Recommendation

**CONDITIONAL GO for v0.3.0.**

The software is validated, the advertisement is now semantically correct,
the release gates pass, the packaging is complete and locally verified, and
the user-facing claims match the hardware evidence. The condition is
procedural: the operator reviews the release notes, then performs the
tagging and publication steps explicitly.

No publication, tag, push or physical print is authorized by this audit.

### Proposed release sequence (operator-controlled)

The full sequence, including the checksum dependency on the tag and the
recovery path for a partial publication, is in
[docs/releases/v0.3.0.md](../releases/v0.3.0.md).
