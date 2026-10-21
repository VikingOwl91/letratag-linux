//! Device geometry and the logical/physical resolution model for the
//! DYMO LetraTag 200B.
//!
//! ## Resolution model (corrected 2026-10-21)
//!
//! The original 2026-10-20 `HELLO LINUX` text-width measurement (~24.3 mm)
//! was erroneous; the corrected width is 12.3 mm, consistent with every
//! subsequent print across the Python/Bleak prototype, Rust/BlueZ and
//! Rust/btleplug transports. Current empirical physical geometry, measured
//! 2026-10-21 (RAW raster diagnostic: 160 feed columns = 12.9 mm; `HELLO
//! LINUX` reproduction: 152 columns = 12.3 mm; calibration rectangle: 252
//! columns = 20.25 mm; cross-tape: 16 rows = 2.25 mm, 30 rows = 4.75 mm):
//!
//! * **Feed axis: ~315 DPI** (empirical estimate, not a specification).
//! * **Cross-tape axis: ~160 DPI** (empirical estimate, not a specification).
//!
//! Two resolutions are distinguished explicitly:
//!
//! * [`LOGICAL_DPI`] (200 x 200) is the document/layout coordinate system
//!   that IPP clients are told to use. It is **not** the printer's native
//!   hardware resolution.
//! * [`CalibrationProfile`] holds the empirical physical raster geometry
//!   (feed and cross-tape DPI, independently configurable). The named
//!   default [`CalibrationProfile::LT200B_2026_10_21`] carries the
//!   2026-10-21 measurements. These values are estimates for calibration,
//!   never presented as manufacturer specifications.
//!
//! ## Physical frame geometry
//!
//! * The wire protocol fixes a 32-dot frame across the tape
//!   ([`letratag_protocol::HEAD_ROWS`]); 30 content rows print, centered
//!   (S1's canvas convention; frame margin 1 row top and bottom).
//! * The printable band is 30 x 25.4/160 = **4.7625 mm** tall, centered on
//!   the 12 mm tape. The active print-row count therefore limits the
//!   practical printable height to ~4.76 mm, below the nominal tape width.
//!   Content outside the band is rejected, never silently cropped.
//! * The 12 mm tape width is a media dimension, not the printable width.
//!   Label length is the media length: 10..200 mm, default 50 mm.
//!
//! ## Conversion discipline
//!
//! Physical dimensions are expressed in millimeters and preserved
//! independently of raster resolution. Raster conversions go through
//! millimeters with explicit, named functions and round-half-away-from-zero
//! rounding, so repeated conversions never accumulate rounding error and
//! logical pixels, physical pixels and millimeters are never mixed
//! silently.

use letratag_protocol::{HEAD_ROWS, MAX_COLUMNS};

/// Logical layout resolution (dots per inch, both axes): the coordinate
/// system documents are laid out in and the resolution advertised to IPP
/// clients. This is a layout convention, not the printer's native hardware
/// resolution; the physical raster geometry is the [`CalibrationProfile`].
pub const LOGICAL_DPI: f64 = 200.0;

/// Empirical physical raster geometry of the printer: feed-axis and
/// cross-tape dot pitch in dots per inch, independently configurable.
///
/// The values are calibration estimates measured on the reference device
/// (2026-10-21), not manufacturer specifications. Adjust them through the
/// calibration procedure (docs/calibration.md); never present them as exact
/// hardware specifications.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CalibrationProfile {
    /// Physical dot pitch along the feed axis (label length direction).
    pub feed_dpi: f64,
    /// Physical dot pitch across the tape (printhead row direction).
    pub cross_dpi: f64,
}

impl CalibrationProfile {
    /// Named default calibration: the 2026-10-21 empirical measurements on
    /// the reference LT200B (~315 DPI feed, ~160 DPI cross-tape).
    pub const LT200B_2026_10_21: Self = Self {
        feed_dpi: 315.0,
        cross_dpi: 160.0,
    };

    /// Scale from logical (200-DPI) pixels to physical feed columns.
    pub fn feed_scale(&self) -> f64 {
        self.feed_dpi / LOGICAL_DPI
    }

    /// Scale from logical (200-DPI) pixels to physical cross-tape rows.
    pub fn cross_scale(&self) -> f64 {
        self.cross_dpi / LOGICAL_DPI
    }

    /// Physical feed columns for a millimeter length (round half away from
    /// zero). Maximum error: half a column.
    pub fn feed_columns(&self, mm: f64) -> usize {
        (mm * self.feed_dpi / 25.4).round() as usize
    }

    /// Physical cross-tape pixels for a millimeter height (round half away
    /// from zero). Maximum error: half a row.
    pub fn cross_pixels(&self, mm: f64) -> usize {
        (mm * self.cross_dpi / 25.4).round() as usize
    }

    /// Logical (200-DPI) pixels for a millimeter length.
    pub fn logical_pixels(&self, mm: f64) -> usize {
        (mm * LOGICAL_DPI / 25.4).round() as usize
    }

    /// Physical feed columns for a logical-pixel width. The conversion goes
    /// through millimeters so the physical dimension is preserved
    /// independently of raster resolution.
    pub fn feed_columns_for_logical(&self, logical_px: usize) -> usize {
        self.feed_columns(f64::from(logical_px as u32) * 25.4 / LOGICAL_DPI)
    }

    /// Physical cross-tape rows for a logical-pixel height (via millimeters).
    pub fn cross_pixels_for_logical(&self, logical_px: usize) -> usize {
        self.cross_pixels(f64::from(logical_px as u32) * 25.4 / LOGICAL_DPI)
    }

    /// Millimeters per physical feed column.
    pub fn feed_mm_per_column(&self) -> f64 {
        25.4 / self.feed_dpi
    }

    /// Millimeters per physical cross-tape row.
    pub fn cross_mm_per_row(&self) -> f64 {
        25.4 / self.cross_dpi
    }

    /// Feed columns for an IPP media length (1/100 mm).
    pub fn media_length_to_feed_columns(&self, media_length: u32) -> usize {
        self.feed_columns(f64::from(media_length) / 100.0)
    }

    /// The largest feed-column count this profile can request for a media
    /// length within [`MIN_LENGTH_MM`]..=[`MAX_LENGTH_MM`].
    pub fn max_media_feed_columns(&self) -> usize {
        self.feed_columns(MAX_LENGTH_MM)
    }

    /// True when a feed-column count fits the protocol's raster bound.
    pub fn feed_columns_within_protocol(&self, columns: usize) -> bool {
        columns <= MAX_COLUMNS
    }
}

/// The default calibration profile used when no override is configured.
pub const DEFAULT_CALIBRATION: CalibrationProfile = CalibrationProfile::LT200B_2026_10_21;

/// Tape width in millimeters. Media dimension, not the printable width.
pub const TAPE_WIDTH_MM: f64 = 12.0;

/// Wire frame height in dots (the protocol's fixed printhead dimension).
pub const FRAME_ROWS: usize = HEAD_ROWS;

/// Content rows printed across the tape (S1's canvas convention), centered
/// within the frame.
pub const PRINTABLE_ROWS: usize = 30;

/// Blank frame rows above and below the content area, centering it.
pub const FRAME_MARGIN_ROWS: usize = (FRAME_ROWS - PRINTABLE_ROWS) / 2;

/// Printable band height in millimeters at the default cross-tape pitch.
/// This is below the 12 mm tape width: the active print-row count limits the
/// practical printable height.
pub fn printable_band_mm(profile: &CalibrationProfile) -> f64 {
    PRINTABLE_ROWS as f64 * profile.cross_mm_per_row()
}

/// Label length bounds in millimeters (software/test media profile values,
/// not measured hardware capabilities).
pub const MIN_LENGTH_MM: f64 = 10.0;
pub const MAX_LENGTH_MM: f64 = 200.0;
pub const DEFAULT_LENGTH_MM: f64 = 50.0;

/// IPP media units are 1/100 mm.
pub const MEDIA_WIDTH: u32 = (TAPE_WIDTH_MM * 100.0) as u32;
pub const MEDIA_MIN_LENGTH: u32 = (MIN_LENGTH_MM * 100.0) as u32;
pub const MEDIA_MAX_LENGTH: u32 = (MAX_LENGTH_MM * 100.0) as u32;
pub const MEDIA_DEFAULT_LENGTH: u32 = (DEFAULT_LENGTH_MM * 100.0) as u32;

/// First rendered page row (0-based, from the top) of the centered
/// printable band for a page `cross` pixels tall.
pub fn band_start(cross: usize) -> usize {
    (cross - PRINTABLE_ROWS) / 2
}

/// Wire frame row for a 0-based row within the printable band. The band
/// maps 1:1 onto wire rows `FRAME_MARGIN_ROWS..FRAME_MARGIN_ROWS +
/// PRINTABLE_ROWS`; the remaining frame rows stay blank.
pub fn wire_row(band_row: usize) -> usize {
    band_row + FRAME_MARGIN_ROWS
}

/// Millimeters for a PDF point length.
pub fn pt_to_mm(pt: f64) -> f64 {
    pt * 25.4 / 72.0
}

/// Parse a `--calibration feed=<dpi>,cross=<dpi>` override. Both axes are
/// required and must be positive and finite; the result is a full profile.
pub fn parse_calibration_override(text: &str) -> Result<CalibrationProfile, String> {
    let mut feed = None;
    let mut cross = None;
    for part in text.split(',') {
        let (key, value) = part.split_once('=').ok_or_else(|| {
            format!("invalid calibration pair {part:?}; expected feed=<dpi>,cross=<dpi>")
        })?;
        let dpi: f64 = value
            .trim()
            .parse()
            .map_err(|_| format!("invalid {key} DPI {value:?}"))?;
        if !dpi.is_finite() || dpi <= 0.0 {
            return Err(format!("{key} DPI must be positive and finite, got {dpi}"));
        }
        match key.trim() {
            "feed" => feed = Some(dpi),
            "cross" => cross = Some(dpi),
            other => return Err(format!("unknown calibration axis {other:?}")),
        }
    }
    Ok(CalibrationProfile {
        feed_dpi: feed.ok_or("missing feed=<dpi>")?,
        cross_dpi: cross.ok_or("missing cross=<dpi>")?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const PROFILE: CalibrationProfile = DEFAULT_CALIBRATION;

    #[test]
    fn feed_lengths_map_to_physical_columns() {
        // 315 DPI: 12.4016 columns per millimeter.
        assert_eq!(PROFILE.feed_columns(10.0), 124);
        assert_eq!(PROFILE.feed_columns(20.0), 248);
        assert_eq!(PROFILE.feed_columns(40.0), 496);
        assert_eq!(PROFILE.feed_columns(50.0), 620);
        assert_eq!(PROFILE.feed_columns(100.0), 1240);
    }

    #[test]
    fn cross_tape_dimensions_within_the_printable_area() {
        // 160 DPI: 6.2992 pixels per millimeter.
        assert_eq!(PROFILE.cross_pixels(12.0), 76);
        assert_eq!(PROFILE.cross_pixels(TAPE_WIDTH_MM), 76);
        // Printable band: 30 rows = 4.7625 mm, below the 12 mm tape.
        assert!((printable_band_mm(&PROFILE) - 4.7625).abs() < 1e-9);
        assert!(printable_band_mm(&PROFILE) < TAPE_WIDTH_MM);
        // A 5 mm region needs 31 cross rows: it exceeds the 30-row band.
        assert_eq!(PROFILE.cross_pixels(5.0), 31);
        assert!(PROFILE.cross_pixels(5.0) > PRINTABLE_ROWS);
    }

    #[test]
    fn millimeter_round_trips_stay_within_half_a_pixel() {
        for mm in [10.0, 12.0, 20.0, 40.0, 50.0, 100.0, 200.0] {
            let cols = PROFILE.feed_columns(mm);
            let back = cols as f64 * PROFILE.feed_mm_per_column();
            assert!(
                (back - mm).abs() <= PROFILE.feed_mm_per_column() / 2.0 + 1e-9,
                "feed round trip {mm} mm -> {cols} cols -> {back} mm"
            );
            let rows = PROFILE.cross_pixels(mm);
            let back = rows as f64 * PROFILE.cross_mm_per_row();
            assert!(
                (back - mm).abs() <= PROFILE.cross_mm_per_row() / 2.0 + 1e-9,
                "cross round trip {mm} mm -> {rows} rows -> {back} mm"
            );
        }
    }

    #[test]
    fn rounding_error_is_bounded_by_half_a_physical_pixel() {
        for mm in [10.0, 24.3, 40.0, 50.0, 63.5, 100.0, 199.9] {
            let cols = PROFILE.feed_columns(mm);
            let exact = mm * PROFILE.feed_dpi / 25.4;
            assert!((cols as f64 - exact).abs() <= 0.5 + 1e-9);
        }
    }

    #[test]
    fn logical_to_physical_mapping_matches_the_mission_example() {
        // 40 mm x 5 mm logical region at 200 DPI:
        // logical 315 x 39 px -> physical 496 x 31 px.
        assert_eq!(PROFILE.logical_pixels(40.0), 315);
        assert_eq!(PROFILE.logical_pixels(5.0), 39);
        assert_eq!(PROFILE.feed_columns_for_logical(315), 496);
        assert_eq!(PROFILE.cross_pixels_for_logical(39), 31);
        // Scale factors: 315/200 = 1.575, 160/200 = 0.8.
        assert!((PROFILE.feed_scale() - 1.575).abs() < 1e-12);
        assert!((PROFILE.cross_scale() - 0.8).abs() < 1e-12);
    }

    #[test]
    fn squares_stay_square_in_physical_space() {
        // A logical square maps to equal physical dimensions within rounding.
        for logical_px in [50usize, 100, 200, 315] {
            let cols = PROFILE.feed_columns_for_logical(logical_px);
            let rows = PROFILE.cross_pixels_for_logical(logical_px);
            let feed_mm = cols as f64 * PROFILE.feed_mm_per_column();
            let cross_mm = rows as f64 * PROFILE.cross_mm_per_row();
            let bound = (PROFILE.feed_mm_per_column() + PROFILE.cross_mm_per_row()) / 2.0;
            assert!(
                (feed_mm - cross_mm).abs() <= bound + 1e-9,
                "logical {logical_px}px -> {cols} cols ({feed_mm} mm) x {rows} rows ({cross_mm} mm)"
            );
        }
    }

    #[test]
    fn orientation_and_axis_mapping_are_explicit() {
        // Feed is the label length (long axis); cross is the tape width.
        // A 50 x 12 mm page: feed = 620 columns, cross = 76 pixels.
        let page = 141.732283_f64 * 25.4 / 72.0;
        let tape = 34.015748_f64 * 25.4 / 72.0;
        assert!((page - 50.0).abs() < 0.01);
        assert!((tape - 12.0).abs() < 0.01);
        assert_eq!(PROFILE.feed_columns(page), 620);
        assert_eq!(PROFILE.cross_pixels(tape), 76);
        // A 12 x 50 mm (portrait) page swaps the roles.
        assert_eq!(PROFILE.cross_pixels(page), 315);
        assert_eq!(PROFILE.feed_columns(tape), 149);
    }

    #[test]
    fn maximum_raster_dimensions_respect_the_protocol_bound() {
        // 200 mm media maximum = 2480 feed columns, within MAX_COLUMNS.
        assert_eq!(PROFILE.max_media_feed_columns(), 2480);
        assert!(PROFILE.feed_columns_within_protocol(2480));
        assert!(PROFILE.feed_columns_within_protocol(MAX_COLUMNS));
        assert!(!PROFILE.feed_columns_within_protocol(MAX_COLUMNS + 1));
        // Media constants stay in 1/100 mm.
        assert_eq!(MEDIA_WIDTH, 1200);
        assert_eq!(MEDIA_MIN_LENGTH, 1000);
        assert_eq!(MEDIA_MAX_LENGTH, 20000);
        assert_eq!(MEDIA_DEFAULT_LENGTH, 5000);
        assert_eq!(
            PROFILE.media_length_to_feed_columns(MEDIA_DEFAULT_LENGTH),
            620
        );
        assert_eq!(PROFILE.media_length_to_feed_columns(MEDIA_MIN_LENGTH), 124);
        assert_eq!(PROFILE.media_length_to_feed_columns(MEDIA_MAX_LENGTH), 2480);
    }

    #[test]
    fn printable_band_is_centered_in_the_frame() {
        assert_eq!(FRAME_ROWS, 32);
        assert_eq!(PRINTABLE_ROWS, 30);
        assert_eq!(FRAME_MARGIN_ROWS, 1);
        assert_eq!(wire_row(0), 1);
        assert_eq!(wire_row(PRINTABLE_ROWS - 1), 30);
        assert_eq!(band_start(76), 23);
        assert_eq!(band_start(76) + PRINTABLE_ROWS, 53);
        assert!(wire_row(PRINTABLE_ROWS - 1) < FRAME_ROWS);
    }

    #[test]
    fn pdf_points_convert_to_millimeters() {
        assert!((pt_to_mm(141.732283) - 50.0).abs() < 0.01);
        assert!((pt_to_mm(34.015748) - 12.0).abs() < 0.01);
        assert!((pt_to_mm(72.0) - 25.4).abs() < 1e-9);
    }

    #[test]
    fn calibration_override_parsing() {
        let profile = parse_calibration_override("feed=315,cross=160").unwrap();
        assert_eq!(profile, PROFILE);
        let custom = parse_calibration_override("feed=314.6,cross=159.5").unwrap();
        assert!((custom.feed_dpi - 314.6).abs() < 1e-9);
        assert!((custom.cross_dpi - 159.5).abs() < 1e-9);
        assert!(parse_calibration_override("feed=315").is_err());
        assert!(parse_calibration_override("cross=160").is_err());
        assert!(parse_calibration_override("feed=0,cross=160").is_err());
        assert!(parse_calibration_override("feed=-1,cross=160").is_err());
        assert!(parse_calibration_override("sideways=1,cross=160").is_err());
        assert!(parse_calibration_override("feed=x,cross=160").is_err());
        assert!(parse_calibration_override("feed=315,cross=160,extra=1").is_err());
    }

    #[test]
    fn empirical_estimates_are_documented_not_exact() {
        // The named profile carries the 2026-10-21 estimates; the measured
        // spread across prints was ~314-316 DPI feed.
        assert_eq!(PROFILE.feed_dpi, 315.0);
        assert_eq!(PROFILE.cross_dpi, 160.0);
        // 152 columns measured 12.3 mm: 313.9 DPI apparent.
        assert!((152.0_f64 * 25.4 / 12.3 - 313.9).abs() < 0.1);
        // 160 columns measured 12.9 mm: 315.0 DPI apparent.
        assert!((160.0_f64 * 25.4 / 12.9 - 315.0).abs() < 0.1);
    }
}
