# Changelog

## Unreleased

### Documentation, 2026-10-21

* Recommend [Direct IPP](docs/direct-ipp.md) as the default workflow through
  the shared PAPPL → Rust → BLE path. The CLI still defaults to mock transport;
  hardware remains an explicit operator choice.
* Mark [CUPS Experimental](docs/cups-experimental.md). Its historical
  readable-output acceptance does not establish dimensional accuracy.
* Retain the `cups-settings` command with Direct IPP guidance and an
  Experimental CUPS warning. Add reusable IPP requests and a mock-only
  client/media regression to CI; no physical acceptance is implied.
* Update the [hardware runbook](docs/hardware-acceptance.md) and
  [verification record](docs/verification.md) to distinguish direct mock
  controls at 50 and 63.5 mm from the physically readable, unmeasured direct
  50 × 12-mm label. Preserve dated historical measurements and release prose.
* Record the [forensic finding](docs/evidence/resolution-investigation-2026-10-21.md):
  CUPS `pdftopdf` shrank artwork by 0.787402 on both axes; a submitted media
  keyword did not establish the actual selected size. The scaling-none
  control is not a complete production fix. Historical ~160 versus ~316-dpi
  apparent feed remains unresolved; no DPI fix or native-pitch proof is claimed.

Only informational CLI text and offline test helpers accompany the
documentation. Rendering, protocol, transport, runtime configuration and
versions are unchanged. No release or new physical acceptance is claimed.
Historical evidence and the authoritative forensic reference are unchanged.

### Release acceptance v0.3.0, 2026-10-21 (not released)

Software gates passed (86 default / 105 all-feature tests, formatting,
Clippy, Direct IPP and PDF-only IPP suites, forensic decoder, offline CUPS
geometry control, documentation links). Two operator-approved physical
Direct IPP prints were readable and correctly oriented, but the calibration
label measured 20.25 x 4.75 mm against a 40.0 x 4.76 mm expectation:
the feed direction is 49.4% short (~316 dpi apparent) while the across
direction matches ~160 dpi. The anomaly reproduces through Direct IPP, so
CUPS is ruled out as its cause; the mechanism remains unresolved and no DPI
compensation was applied. Recommendation: NO-GO for dimensional-accuracy
claims. Evidence:
[release readiness report](docs/evidence/release-readiness-v0.3.0-2026-10-21.md).
No version bump, tag, publication or release was made.

### BLE transport, 2026-10-21 (unreleased)

* Add a selectable [btleplug](docs/btleplug-migration-plan.md) BLE backend
  (pinned 0.11.8) behind the existing transport abstraction, with a
  mockable GATT boundary and deterministic regression tests. `serve
  --transport mock|bluez|btleplug`; `hardware-send --transport
  bluez|btleplug` (default bluez, unchanged). Mock stays the default;
  Engine, journaling, protocol, rendering, Direct IPP and the Experimental
  CUPS labeling are unchanged. A connect-only comparison found identical
  payloads, GATT operations and write mode on both backends; see
  [backend comparison](docs/evidence/backend-comparison-2026-10-21.md).
  The feed-pitch discrepancy is not assumed fixed by the library change.
* Hardware acceptance: one operator-authorized HELLO LINUX print via
  btleplug was readable and correctly oriented; text width 12.3 mm,
  identical to the BlueZ backend. Decision: retain both backends, BlueZ
  remains the default. See
  [hardware acceptance](docs/evidence/btleplug-hardware-acceptance-2026-10-21.md).
