# Calibration procedure: LetraTag 200B physical geometry

The rendering pipeline uses a **logical 200 x 200 DPI layout** coordinate
system and maps it to the printer's empirical physical raster geometry via
a named, independently configurable [`CalibrationProfile`](../crates/letratag-printer/src/geometry.rs)
(feed DPI, cross-tape DPI). The profile values are **empirical estimates
measured on the reference device (2026-10-21), not manufacturer
specifications**. This document describes how to fit and adjust them.

Current default (`CalibrationProfile::LT200B_2026_10_21`):

* Feed axis: **315 DPI** (152 columns = 12.3 mm; 160 columns = 12.9 mm;
  252 columns = 20.25 mm).
* Cross-tape axis: **160 DPI** (16 rows = 2.25 mm; 30 rows = 4.75 mm).

## Why the raster is asymmetric

The two axes are set by different parts of the device and are not expected
to match. Across the tape, the frame height is fixed: 32 transmitted rows
span the 12 mm tape width, which puts the cross-tape pitch at roughly
160 DPI (30 active rows = 4.75 mm), while the printhead covers only a
narrow band of that width. Along the feed, the pitch comes from how far
the tape advances per column, measured at roughly 315 DPI (152 columns =
12.3 mm). A square, isotropic 200 x 200 DPI raster was never a device
property; 200 DPI is this project's logical layout convention for
documents, and the physical raster is what the printer actually marks.
Both are exposed to IPP clients (see [direct-ipp.md](direct-ipp.md)).

## The diagnostic

`crates/letratag-printer/examples/calibration-label.rs` generates a
110 x 12 mm label PDF with one horizontal reference line per feed length
(10, 20, 40, 50, 100 mm), each exactly `feed_columns(L)` columns at the
configured profile, with end ticks. Wire rows 3/9/15/21/27 carry the
10/20/40/50/100 mm references respectively.

```sh
# Generate and verify offline (no printing):
cargo run -p letratag-printer --features pappl --example calibration-label -- --check

# Write the PDF:
cargo run -p letratag-printer --features pappl --example calibration-label -- \
  --output test-output/calibration-label.pdf
```

`--check` renders the PDF through the document path and verifies the raster
width (1364 columns for the 110 mm page) and every reference line's ink
extent against the commanded columns. It never prints.

## Fitting the feed pitch

1. Print **one** calibration label through the Direct IPP path with explicit
   media (110 mm x 12 mm), only with explicit operator authorization:
   `lp -h 127.0.0.1:8631 -d letratag -o media=custom_110x12mm ...` or the
   equivalent `media-col` (x-dimension=1200, y-dimension=11000).
2. Measure each reference line's ink length between its end ticks
   (calipers, 0.05 mm).
3. For each length L with measured width W: `feed_dpi = columns(L) / W x 25.4`.
4. Compare the five estimates. A **constant-DPI model** is adequate when
   the estimates agree within measurement precision; a systematic trend
   (e.g. larger error at longer lengths) suggests a **fixed offset** in
   addition to the pitch: `W = columns x pitch + offset`. Never infer an
   offset from a single measurement; use at least the 10 mm and 100 mm
   references (a two-point fit only bounds the model; three or more points
   test it).
5. Set the fitted values with `--calibration feed=<dpi>,cross=<dpi>`
   (serve/mock/preview) or update the named profile, regenerate the
   diagnostic, and repeat until residuals are within measurement precision.

## What is calibrated separately (and must not be conflated)

1. **Printed ink geometry** — the reference lines above. Controlled by
   `feed_dpi` (and `cross_dpi` for the band rows).
2. **Leading/trailing blank feed** — the margins between the cut edge and
   the first/last ink. Not controlled by the ink calibration; measure and
   record separately.
3. **Total physical label/cut length** — the cut-to-cut length. The printer
   adds its own lead-in; a correction to ink width must not be assumed to
   correct total cut length.

Record commanded columns, measured lengths, the fitted profile, and the
residuals in a dated evidence document under `docs/evidence/`.

## Cross-tape calibration

The cross-tape pitch (~160 DPI) is measured with the printable band height
(30 rows = 4.7625 mm at 160 DPI) and any known-height feature. Adjust
`cross_dpi` independently. The active print-row count limits the printable
height to 30 rows (~4.76 mm at 160 DPI), below the 12 mm tape width;
content outside the band is rejected, never silently cropped.

## Feed offsets and row positioning

Only add a feed-direction offset or row-positioning adjustment when the
residuals justify it, with the same multi-length discipline. Prefer the
simplest auditable model (constant DPI) over speculative firmware
compensation.

## Limits

* Software tests verify the mapping, never physical accuracy; only
  measurement on hardware establishes dimensional accuracy.
* The 2026-10-20 historical total-length records (62-66.5 mm for
  400-column labels) remain in the evidence archive and are not reconciled
  by the corrected text-width measurement; the profile targets current
  observed geometry.
* This procedure was executed once, on 2026-10-21, with one operator-
  authorized label: reference ink lengths from 10 mm to 100 mm came back
  consistent with the constant-DPI model at the precision of the readings
  (reported to the nearest millimetre). Evidence:
  [hardware validation](evidence/hardware-validation-2026-10-21.md). That
  is one device, one label and the feed axis; it is not a general
  dimensional-accuracy guarantee, and the profile stays an empirical
  calibration that can be refitted with `--calibration`.
* Record the reading precision with every measurement. A calculated model
  residual is not a measurement error, and residuals far below the reading
  precision say nothing about physical accuracy.
