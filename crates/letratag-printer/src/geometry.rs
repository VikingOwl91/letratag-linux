//! Device resolution and physical geometry for the DYMO LetraTag 200B.
//!
//! Resolution model (verified 2026-10-20/21, see docs/protocol.md section 6
//! and docs/evidence/hardware-calibration-batch-2026-10-20.md):
//!
//! * The dot pitch is ~160 dpi isotropic. Feed: 152 text columns = 24.3 mm
//!   = 158.9 dpi (ruler, operator-confirmed). Across: 16 text rows =
//!   2.25-2.5 mm (2.54 mm expected at 160 dpi; within caliper precision).
//!   This matches the manufacturer's 160 DPI specification.
//! * The 200 dpi value in the S2 device file and the old reference template
//!   is a source claim, refuted by measurement: content laid out at
//!   200 dpi printed 1.25x too long along the feed on the physical device.
//! * The wire protocol fixes a 32-dot frame across the tape
//!   ([`letratag_protocol::HEAD_ROWS`]); 30 content rows are printed,
//!   centered (S1's canvas convention). At 160 dpi the printed band is
//!   32 dots = 5.08 mm, centered on the 12 mm tape; the content area is
//!   30 dots = 4.76 mm.
//! * The 12 mm tape width is a media dimension, not the printable width.
//!   Label length is the media length: 10..200 mm, default 50 mm.
//!
//! All resolution-dependent conversions in the pipeline derive from these
//! constants; nothing here is a rendering assumption independent of the
//! measured device behavior.

use letratag_protocol::HEAD_ROWS;

/// Device dot pitch in dots per inch, measured on the hardware (2026-10-20)
/// and matching the manufacturer's 160 DPI specification.
pub const RESOLUTION_DPI: f64 = 160.0;

/// Tape width in millimeters. Media dimension, not the printable width.
pub const TAPE_WIDTH_MM: f64 = 12.0;

/// Wire frame height in dots (the protocol's fixed printhead dimension).
pub const FRAME_ROWS: usize = HEAD_ROWS;

/// Content rows printed across the tape (S1's canvas convention), centered
/// within the frame.
pub const PRINTABLE_ROWS: usize = 30;

/// Blank frame rows above and below the content area, centering it.
pub const FRAME_MARGIN_ROWS: usize = (FRAME_ROWS - PRINTABLE_ROWS) / 2;

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

/// Pixels (dots) for a millimeter length at the device resolution.
pub fn mm_to_dots(mm: f64) -> f64 {
    mm * RESOLUTION_DPI / 25.4
}

/// Millimeters for a dot count at the device resolution.
pub fn dots_to_mm(dots: usize) -> f64 {
    f64::from(dots as u32) * 25.4 / RESOLUTION_DPI
}

/// Pixels (dots) for a PDF point length at the device resolution.
pub fn pt_to_dots(pt: f64) -> f64 {
    pt * RESOLUTION_DPI / 72.0
}

/// Feed dots for an IPP media length (1/100 mm).
pub fn media_length_to_dots(media_length: u32) -> usize {
    (f64::from(media_length) * RESOLUTION_DPI / 2540.0).round() as usize
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn millimeter_dot_conversions_match_the_measured_resolution() {
        // 50 mm at 160 dpi = 314.96 dots (renders as 315 px).
        assert!((mm_to_dots(50.0) - 314.96).abs() < 0.01);
        // 12 mm tape = 75.59 dots (renders as 76 px).
        assert!((mm_to_dots(12.0) - 75.59).abs() < 0.01);
        // 200 mm maximum length = 1259.84 dots.
        assert!((mm_to_dots(200.0) - 1259.84).abs() < 0.01);
        // 10 mm minimum length = 62.99 dots.
        assert!((mm_to_dots(10.0) - 62.99).abs() < 0.01);
        // 40 mm calibration reference = 251.97 dots.
        assert!((mm_to_dots(40.0) - 251.97).abs() < 0.01);
        // Inverse conversion.
        assert!((dots_to_mm(315) - 50.01).abs() < 0.01);
        assert!((dots_to_mm(30) - 4.76).abs() < 0.01);
        // 200 dpi would give 394 dots for 50 mm: a different model.
        assert!((50.0 * 200.0 / 25.4 - 393.70f64).abs() < 0.01);
    }

    #[test]
    fn pdf_point_conversions_match_the_page_geometry() {
        // 50 x 12 mm page = 141.732283 x 34.015748 pt.
        assert!((pt_to_dots(141.732283) - 314.96).abs() < 0.01);
        assert!((pt_to_dots(34.015748) - 75.59).abs() < 0.01);
    }

    #[test]
    fn ipp_media_lengths_convert_to_feed_dots() {
        assert_eq!(media_length_to_dots(MEDIA_DEFAULT_LENGTH), 315);
        assert_eq!(media_length_to_dots(MEDIA_MIN_LENGTH), 63);
        assert_eq!(media_length_to_dots(MEDIA_MAX_LENGTH), 1260);
        assert_eq!(media_length_to_dots(5000), 315);
    }

    #[test]
    fn ipp_media_constants_use_hundredths_of_millimeters() {
        assert_eq!(MEDIA_WIDTH, 1200);
        assert_eq!(MEDIA_MIN_LENGTH, 1000);
        assert_eq!(MEDIA_MAX_LENGTH, 20000);
        assert_eq!(MEDIA_DEFAULT_LENGTH, 5000);
    }

    #[test]
    fn printable_band_is_centered_in_the_frame() {
        assert_eq!(FRAME_ROWS, 32);
        assert_eq!(PRINTABLE_ROWS, 30);
        assert_eq!(FRAME_MARGIN_ROWS, 1);
        // Content maps 1:1 onto wire rows 1..=30; rows 0 and 31 stay blank.
        assert_eq!(wire_row(0), 1);
        assert_eq!(wire_row(PRINTABLE_ROWS - 1), 30);
        // A 12 mm page renders 76 px tall; the band is rows 23..53.
        assert_eq!(band_start(76), 23);
        assert_eq!(band_start(76) + PRINTABLE_ROWS, 53);
        // The band never exceeds the frame.
        assert!(wire_row(PRINTABLE_ROWS - 1) < FRAME_ROWS);
    }

    #[test]
    fn resolution_is_the_measured_device_pitch() {
        assert_eq!(RESOLUTION_DPI, 160.0);
        // The measured feed pitch was 158.9 dpi; 160 is the verified model.
        assert!((152.0 * 25.4 / 24.3 - 158.9f64).abs() < 0.1);
    }
}
