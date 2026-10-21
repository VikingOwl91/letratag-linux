# Hardware validation plan: calibrated 200-DPI rendering, 2026-10-21

**No physical print is sent by this plan. It executes only after explicit
operator authorization.** The v0.3.0 dimensional-accuracy NO-GO stands until
this plan's measurements demonstrate acceptable results.

## Objective

Confirm on hardware that a requested physical dimension maps predictably to
the native raster geometry through the calibrated pipeline: printed ink
lengths against known requested lengths, consistency across multiple
lengths, printable-height constraints, and leading/trailing feed versus
total cut length (measured separately).

## Test design (one label, minimal tape)

One calibration label (110 x 12 mm) with five horizontal reference lines at
known commanded lengths (docs/calibration.md):

| Wire row | Reference length | Commanded columns (default profile) | Expected ink (mm) |
|---|---|---|---|
| 3 | 10 mm | 124 | 9.9987 |
| 9 | 20 mm | 248 | 19.9975 |
| 15 | 40 mm | 496 | 39.9949 |
| 21 | 50 mm | 620 | 49.9937 |
| 27 | 100 mm | 1240 | 99.9873 |

The label is 1364 feed columns (110 mm at 315 DPI) x 76 cross-tape pixels
(12 mm at 160 DPI), printable band rows 23..52 (4.7625 mm). Offline
verification already passed: `calibration-label --check` (raster width and
every line's ink extent at the commanded columns).

### Procedure

1. Operator confirms readiness (printer on, lid closed, 12 mm cassette
   seated, batteries charged).
2. Build with `pappl,ble` (BlueZ backend, the default real-hardware backend).
3. Start the endpoint:
   `target/debug/letratag-printer serve --state test-output/hw-val-state \
    --spool test-output/hw-val-spool --port 8631 --transport bluez \
    --address 90:E5:B1:49:DA:AE --profile legacy --diagnostics`
   (fresh private state seeded with the validated 244 cap).
4. Generate the label:
   `target/debug/examples/calibration-label --output test-output/calibration-label.pdf`
5. Submit exactly one job via Direct IPP with explicit media-col
   (1200 x 11000, scaling none, one copy):
   `ipptool -d length=11000 -f test-output/calibration-label.pdf \
    ipp://127.0.0.1:8631/ipp/print examples/print-pdf.test`
6. Record diagnostics (write sequence, timing, status) and the engine
   outcome (uncertain by design). No retries, no additional prints.
7. Operator measures with calipers (0.05 mm):
   - each reference line's ink length between its end ticks;
   - the printable band height (ink extent across the tape);
   - leading blank (cut edge to first ink), trailing blank, and total
     cut-to-cut length (recorded separately from ink geometry).
8. Archive the journal with the operator decision.

### Acceptance evaluation

* For each reference: `feed_dpi = columns / measured_mm x 25.4`. The five
  estimates must agree within measurement precision for a constant-DPI
  model; a systematic trend indicates a fixed offset (never infer an offset
  from a single measurement).
* Compare against the profile default (315 DPI). Update the profile only
  from the fitted values; document residuals.
* Band height: 30 rows at the cross-tape pitch (expected ~4.76 mm at
  160 DPI).
* Total cut length is evaluated separately from ink geometry.

### Interpretation rules

* Ink lengths matching the requested lengths within measurement precision:
  the calibration model is confirmed; dimensional-accuracy acceptance can
  be reconsidered.
* Systematic deviation: fit and adjust the profile (docs/calibration.md);
  repeat only with new explicit authorization.
* Any other result: investigate without assuming a mechanism.

Software-only tests never establish physical dimensional accuracy.
