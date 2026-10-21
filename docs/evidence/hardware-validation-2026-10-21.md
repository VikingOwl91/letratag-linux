# Hardware validation of calibrated 200-DPI rendering, 2026-10-21

First hardware confirmation that a requested physical dimension maps
predictably to the native raster geometry through the calibrated pipeline.
Executed per [hardware-validation-plan-2026-10-21.md](hardware-validation-plan-2026-10-21.md)
after explicit operator authorization. Exactly one print; no retries.

## Procedure and transmission

* Label: `examples/calibration-label.rs` output (110 x 12 mm PDF), verified
  offline first (`--check`: raster 1364 columns; five reference lines at
  commanded columns 124/248/496/620/1240).
* Endpoint: `serve --transport bluez` (default real-hardware backend),
  legacy profile, validated 244-byte cap, fresh private journal
  `test-output/hw-val-state`.
* One job via Direct IPP with explicit media-col (x-dimension=1200,
  y-dimension=11000), print-scaling none, one copy
  (`examples/print-pdf.test`), submitted 2026-10-21T06:56:19Z.
* Diagnostics: 24 writes `[9,242 x22,181]` (legacy body 5480 =
  24 + 4 x 1364); Write Request; effective size 244; status `1b 52 00` at
  22.84 s (18.19 s after the last write); engine outcome Uncertain at
  52.84 s by design. Journal `1792565782021715291.state` archived with the
  operator decision.

## Operator results

Label emerged; five reference lines visible; orientation correct.
Calipers (0.05 mm):

| Wire row | Reference | Commanded columns | Model ink (mm) | Measured (mm) | Residual (mm) | Implied feed DPI |
|---|---|---|---|---|---|---|
| 3 | 10 mm | 124 | 9.9987 | 10 | -0.0013 | 314.96 |
| 9 | 20 mm | 248 | 19.9975 | 20 | -0.0025 | 314.96 |
| 15 | 40 mm | 496 | 39.9949 | 40 | -0.0051 | 314.96 |
| 21 | 50 mm | 620 | 49.9937 | 50 | -0.0063 | 314.96 |
| 27 | 100 mm | 1240 | 99.9873 | 100 | -0.0127 | 314.96 |

* Printable band height: ~4.75 mm measured; model 4.7625 mm (30 rows at
  160 DPI cross-tape) -> implied 160.4 DPI. Confirmed.
* Leading blank (cut edge to first ink): 19 mm = 10 mm raster leading
  margin (124 columns) + ~9 mm printer lead-in. Consistent with earlier
  observations (~9.2 mm).
* Trailing blank: ~9 mm.
* Total cut-to-cut: ~127 mm = 110 mm content + ~9 mm lead-in + ~9 mm
  lead-out (model 128 mm).

## Evaluation

* **Constant-DPI model confirmed across 10-100 mm.** All five implied feed
  pitches are 314.96 DPI (the 0.04 DPI spread from 315.00 is the
  rounding of the commanded columns, not a measurement trend). Residuals
  are <= 0.013 mm, within caliper precision (0.05 mm). No fixed offset is
  needed; none is inferred.
* **Cross-tape pitch confirmed** at ~160 DPI (band 4.75 vs 4.7625 mm).
* **Ink geometry, blank feed and total cut length are consistent but
  distinct**: the ~9 mm lead-in/lead-out are printer behavior outside the
  ink calibration, as documented in docs/calibration.md.
* Text/graphic proportions are preserved by construction (anisotropic
  mapping preserves physical proportions; verified by unit tests) and the
  reference lines themselves verify the feed axis physically.

## Consequences

* The default calibration profile (`LT200B_2026_10_21`: feed 315 /
  cross 160) is **hardware-validated** within measurement precision.
* This is the first physical validation of dimensional accuracy in the
  project. The v0.3.0 dimensional-accuracy NO-GO can be reconsidered on this
  evidence; the operator decides.
* Remaining limits: one label, one device state; the profile remains an
  empirical calibration (docs/calibration.md) adjustable with
  `--calibration`; total cut length includes printer lead-in/lead-out that
  the ink calibration does not control.
