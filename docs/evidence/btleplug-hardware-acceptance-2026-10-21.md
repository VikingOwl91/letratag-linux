# btleplug hardware acceptance and migration decision, 2026-10-21

Phase 6 hardware acceptance and Phase 7 migration decision for the
btleplug BLE backend migration (plan: [../btleplug-migration-plan.md](../btleplug-migration-plan.md)).

## Hardware acceptance (one authorized print)

Exactly one `HELLO LINUX` print through the btleplug backend, authorized by
the operator after device readiness confirmation (on, lid closed, 12 mm
cassette seated, batteries charged; printer advertising on BLE):

```sh
target/debug/letratag-printer hardware-send test-output/hello-linux.png \
  --address '90:E5:B1:49:DA:AE' --state test-output/btleplug-hw-state \
  --transport btleplug --columns 400 --profile legacy --max-write 244 \
  --linger-seconds 20 --acknowledge-unverified-hardware --diagnostics
```

Submitted 2026-10-21T05:54:10Z. (An earlier invocation at 05:53:44Z failed
before any BLE activity because the debug binary had been rebuilt without
BLE features by the IPP test scripts; it created no journal and transmitted
nothing.)

Diagnostics: transport btleplug; Write mode Request; negotiated ATT MTU
unknown; usable GATT value unknown (not exposed by btleplug on Linux);
validated cap 244; effective write size 244; writes
`[9,242,242,242,242,242,242,181]` — identical to the BlueZ reproduction and
the 2026-10-20 attempt 7 trace. Inter-write interval ~112 ms, matching the
BlueZ backend. Status `1b 52 00` at 9.91 s (7.16 s after the last write,
same as BlueZ). Engine outcome Uncertain at 39.91 s by design. No retries.
Journal `1792562052679771409.state` archived with the operator decision;
GATT evidence in `test-output/btleplug-hw-state/run.log`.

Payload: byte-identical to the retained historical reproduction — body
SHA-256 `58272474123e691ac5d206d11cc91c677efc1b5119a346bb1c7451514d069c53`,
wire SHA-256 `357a6b8dad5a11ca68a39b9825cee3d91551c195f69d7065e07cef0df0eeb98b`,
body 1624 bytes, raster 152 x 16 occupied ink pixels.

Operator confirmation: label emerged, `HELLO LINUX` readable, orientation
correct. **Operator measurement: text width 12.3 mm** for 152 occupied
columns.

## Interpretation (per the mission's decision rule)

* ~12.3 mm: **btleplug reproduces current behavior; the library change
  alone does not resolve the discrepancy.** Confirmed: the btleplug print
  measured exactly the same 12.3 mm as the BlueZ backend's 2026-10-21
  reproduction, and matches the RAW raster diagnostic (~315 dpi apparent
  feed) and the Direct IPP calibration (~316 dpi apparent).
* ~24.3 mm would have indicated a backend-dependent difference. Not
  observed.
* No other result; no mechanism assumed.

**Secondary diagnostic goal answered: the BLE transport implementation does
not contribute to the apparent feed-pitch discrepancy at the application
and GATT-operation level.** Both backends produce byte-identical payloads,
identical write sequences and timings (~112 ms inter-write, 7.16 s
status delay), and identical physical results (12.3 mm). The remaining
difference is the D-Bus client library (bluer vs bluez-async) beneath an
identical GATT operation trace. The discrepancy therefore persists in the
device/firmware or physical feed behavior, not in either transport.

## Migration decision: **B — retain both backends; BlueZ remains the default**

Evidence for the decision:

* **Regression: PASS.** Default workspace 90 tests, all-features 124 tests,
  zero failed; clippy `-D warnings` and rustfmt clean; both IPP suites pass.
  New deterministic coverage proves byte-exact preservation of the retained
  HELLO LINUX and RAW diagnostic payloads, exact GATT operation traces with
  Write Request mode, profile validation, discovery/status/write timeouts,
  mid-transfer failure without replay, oversized-write rejection, and
  Engine journal semantics over the mock GATT boundary.
* **Controlled comparison: PASS (no physical print).** Same device, GATT
  table, characteristics, write mode, payload hashes and effective write
  size on both backends; only connect/disconnect timings and the unexposed
  GATT value limit differ. See
  [backend comparison](backend-comparison-2026-10-21.md).
* **Hardware acceptance: PASS for operation; identical physical result.**
  One authorized btleplug print: readable, correct orientation, 12.3 mm —
  the same as the BlueZ backend today.

Why not A (adopt btleplug as default) yet:

* The physical feed-pitch discrepancy is unresolved. Although the backends
  are application-level equivalent and produce identical physical output
  today, the conservative choice while a hardware-path investigation is
  open is to keep the established, longer-validated backend as the default.
* btleplug exposes no GATT value limit on Linux, so diagnostics report it
  unknown (slightly less observability than bluer's 495 report). The
  effective write size is unaffected (validated cap 244).
* One hardware print is sufficient for acceptance of the backend as an
  equivalent alternative, not yet for a default switch of a hardware path
  with known unresolved physical behavior.

Why not C (retain only BlueZ): the btleplug backend passed regression,
comparison and hardware acceptance with no unacceptable regressions, and
it reduces long-term custom-transport maintenance. It stays available for
controlled comparison (`--transport btleplug`, `serve --transport
btleplug`).

**Upgrade path to A:** once the feed-pitch discrepancy is resolved (or
after an operator-defined operational period with btleplug), switching the
default is a one-line CLI default change; both backends remain compiled and
selectable either way.

## Trade-offs and remaining limitations

* btleplug adds a pinned dependency tree (btleplug 0.11.8, bluez-async 0.8,
  dashmap, ...); the vendored offline build was regenerated.
* btleplug on Linux cannot add a peripheral by address without scanning;
  discovery is scan-based (bounded), matching the BlueZ backend's
  discovery-resolution behavior.
* No negotiated-MTU or GATT-value-size reporting under btleplug; the
  validated 244-byte cap remains the effective limit in both backends.
* No link-layer captures exist; equivalence is established at the
  application-payload and GATT-operation level, plus one identical physical
  result.
* The feed-pitch discrepancy is unchanged by this migration and remains the
  open release blocker for dimensional-accuracy claims (see
  [release readiness](release-readiness-v0.3.0-2026-10-21.md)).
