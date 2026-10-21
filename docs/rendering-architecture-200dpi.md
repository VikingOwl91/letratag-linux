# Architecture investigation and implementation plan: calibrated 200-DPI logical rendering

Status: plan (pre-implementation), 2026-10-21.

## Measurement basis (operator-corrected)

The original 2026-10-20 `HELLO LINUX` text-width measurement (~24.3 mm) was
erroneous; the corrected width is **12.3 mm**, consistent with every
subsequent print. Three transport implementations (Python/Bleak prototype,
Rust/BlueZ, Rust/btleplug) produce the same physical result. Current
empirical physical geometry, measured 2026-10-21 across RAW, Direct IPP,
CUPS and both BLE backends:

* **Feed axis: ~314-315 DPI** (152 cols = 12.3 mm; 160 cols = 12.9 mm;
  252 cols = 20.25 mm).
* **Cross-tape axis: ~160 DPI** (16 rows = 2.25 mm; 30 rows = 4.75 mm).

These are empirical estimates, not manufacturer specifications. Per the
mission, the historical 160-to-315-DPI transition is not treated as an
established phenomenon; the model targets the observed physical geometry.
(Note for the record: the 2026-10-20 total-length records of 62-66.5 mm for
400-column labels remain in the historical evidence archive and are not
reconciled by the corrected text-width narrative; the calibration model
targets current observed geometry and Phase 7 physical validation confirms
it.)

## Phase 1 findings: current architecture

### Where logical dimensions and DPI assumptions live

* `crates/letratag-printer/src/geometry.rs`: the square-pixel model.
  `RESOLUTION_DPI = 160.0` drives `mm_to_dots`, `dots_to_mm`, `pt_to_dots`,
  `media_length_to_dots`. Tape 12 mm, frame 32 rows, 30 printable rows,
  media 1200 x 1000..20000 (1/100 mm), default 5000.
* `document.rs` (PDF path): Poppler `pdftoppm -r 160` renders the page;
  orientation is detected from the rendered PNG dimensions; the centered
  30-row band is extracted; ink outside the band is a `Geometry` error
  (no silent crop). Raster width = feed columns.
* `letratag-render` (PNG path): `render_png`/`render_gray` fit-to-box scaling
  into `RenderOptions { width, active_rows: 30, threshold, dither }`,
  centered in the 32-row frame. This is a physical-raster tool: the caller
  specifies target feed columns explicitly.
* `ffi.rs` / `native/adapter.c` (IPP): `Config.dpi = 160` is advertised as
  `x_resolution`/`y_resolution` (square); `lt_rs_pdf` checks
  `media_width == 1200`, media length range, and
  `|raster.width - media_length * dpi / 2540| <= 1`.

### Where paths converge

PDF and PNG converge at `letratag_protocol::Raster` (width x 32) and then at
the Engine. The IPP path (PAPPL -> C adapter -> `lt_rs_pdf` ->
`document::pdf`) is the only path that interprets media dimensions.

### Square-pixel assumptions

* `geometry.rs` conversions are isotropic (one DPI for both axes).
* `document.rs` renders at a single `-r 160` and treats rows/columns as
  square.
* `adapter.c` advertises square resolution.
* `letratag-render`'s fit-to-box treats source pixels as square.

### Double-scaling hazards

* The PNG path's fit-to-box is an explicit user contract (`--columns`); the
  retained `HELLO LINUX` fixture depends on it byte-exactly.
* The PDF path currently has exactly one scaling step (Poppler at 160 dpi);
  the media check re-derives dots from mm at the same DPI, so no
  double-scaling exists today. The new model must keep exactly one
  resolution-conversion boundary.

### 32-row frame / 30 active rows

The wire frame is 32 rows; 30 rows print, centered (frame margin 1 row
top/bottom). The printable band height is 30 x 25.4/160 = **4.7625 mm**,
which is below the 12 mm tape width: the active print-row count limits the
practical printable height to ~4.76 mm, centered on the tape. Content
outside the band is rejected (PDF path), never silently cropped.

### Feed-column preservation

Feed columns are preserved through encoding (ESC D width = raster width,
4 bytes per column, no resampling in `letratag-protocol`). Transports chunk
the already-final raster; no transport resamples.

## Design decisions

### Logical vs physical resolution (geometry.rs)

* `LOGICAL_DPI = 200.0`: the document/layout coordinate system, and the
  resolution advertised to IPP clients. **Not** the native hardware
  resolution.
* `CalibrationProfile { feed_dpi, cross_dpi }`, independently configurable;
  named default `CalibrationProfile::LT200B_2026_10_21` (315/160), documented
  as empirical estimates measured 2026-10-21, not specifications.
* Conversions are mm-based with explicit names and round-half-away-from-zero
  rounding: `feed_columns(mm)`, `cross_pixels(mm)`, `logical_pixels(mm)`,
  `feed_columns_for_logical(px)`, `cross_pixels_for_logical(px)`,
  `feed_mm_per_column()`, `cross_mm_per_row()`. Physical dimensions are
  preserved independently of raster resolution; logical->physical mapping
  goes through millimeters so rounding never accumulates across repeated
  conversions.

### Rendering boundary (document.rs)

The PDF path renders **directly at the target physical raster resolution**
using `pdftoppm -scale-to-x <feed cols> -scale-to-y <cross px>` (verified:
exact anisotropic output dimensions; no resampling of rasterized content).
Page size comes from `pdfinfo` (`Page 1 size: W x H pts`, `Page 1 rot: N`);
all four orientation/rotation cases map the page axes to physical scales so
the printed output preserves physical proportions. One conversion, one
render, no double-scaling. Band extraction, thresholding, ink-outside-band
rejection and portrait raster rotation are unchanged in behavior.

### PNG path (unchanged, documented)

`letratag-render`'s fit-to-box path is a physical-raster tool (explicit
`--columns`); the retained `HELLO LINUX` fixture depends on it byte-exactly
and the mission forbids altering historical fixtures. The logical model
applies to the document (PDF/IPP) path; PNG users compute columns from
millimeters via `CalibrationProfile::feed_columns` (documented in
`docs/direct-ipp.md`). The paths produce consistent physical geometry for
the same physical label (tested).

### IPP advertisement (Phase 4)

* Advertise the **logical** 200 x 200 DPI as `printer-resolution` (the layout
  coordinate system Direct IPP clients should use). This is legitimate: the
  pipeline renders logical layouts to dimensionally accurate physical
  output. It is documented as logical, not native.
* Expose the physical calibration as a custom printer attribute
  `letratag-physical-resolution` (resolution, feed x cross DPI) so clients
  can observe the empirical physical geometry.
* The media check in `lt_rs_pdf` uses the **physical** feed DPI via the
  profile; the native raster dimensions stay distinct from logical
  document dimensions. Page fitting cannot override requested physical
  dimensions (print-scaling=none is the only supported mode; mismatches
  abort with an actionable error).

> Superseded 2026-10-21 (v0.3.0 release audit): the two bullets above are
> the plan as written before implementation. The shipped behaviour puts the
> **physical** geometry (315 x 160 dpi) in the standard `printer-resolution`
> attribute, because that attribute means the resolution at which the
> printer prints (RFC 8011), and moves the logical layout resolution to the
> custom `letratag-logical-resolution` attribute. The custom
> `letratag-physical-resolution` attribute was dropped as redundant with the
> standard attribute. The plan text is kept unchanged as the record of the
> decision at planning time.

### CLI

`serve`, `mock` and `preview` gain `--calibration feed=<dpi>,cross=<dpi>`
(default: the named profile) for calibration experiments. `hardware-send`
is unchanged (physical-raster PNG path).

## Phase 5 test plan (deterministic)

* Feed lengths 10/20/40/50/100 mm -> 124/248/496/620/1240 columns.
* Cross-tape: 12 mm -> 76 px; band rows 23..53 of 76; printable height
  4.7625 mm < 12 mm tape (documented constraint).
* Round trips mm<->raster within 0.5 physical pixel; rounding error bounds.
* Mission example: 40 x 5 mm logical region -> logical 315 x 39 px ->
  physical 496 x 31 px.
* Axis/orientation mapping incl. `/Rotate 90`; squares stay square in mm.
* PDF path: raster width == feed_columns(page mm) (no double-scaling);
  ink outside the band rejected (no silent crop); oversize page rejected.
* Protocol encoding of unchanged native-raster fixtures stays byte-exact
  (transport crate `payload_preservation` tests, untouched).
* Consistent geometry PDF vs PNG path for the same physical label.
* IPP suites updated to the new model (620 columns for the 50 mm synthetic
  label); capability assertions for the new advertisement.

## Phase 6 calibration (docs/calibration.md + diagnostic)

`examples/calibration-label.rs` generates a label PDF with horizontal
reference lines at 10/20/40/50/100 mm feed lengths (commanded columns
recorded), renderable through the mock for verification. Procedure:
print one calibration label (awaiting explicit authorization), measure each
ink length, fit `feed_dpi = columns / measured_mm x 25.4` per length, and
evaluate constant-DPI vs fixed-offset models on the residuals (an offset is
never inferred from a single measurement). Adjust feed DPI, cross DPI,
feed offset and row positioning independently; distinguish printed ink
geometry, leading/trailing blank feed, and total cut length (a correction
to ink width does not automatically correct cut length).

## Phase 7

Full software gates (fmt, clippy, default/all-features tests, both IPP
suites, forensic/protocol regressions, doc/fixture checks). Hardware
validation plan prepared; **no physical print without explicit operator
authorization**. The v0.3.0 dimensional-accuracy NO-GO is preserved until
physical validation demonstrates acceptable results.
