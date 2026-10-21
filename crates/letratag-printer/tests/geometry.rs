//! Deterministic geometry tests: physical dimensions map predictably to
//! native raster geometry, independently of transport behavior.
//!
//! These tests verify the logical/physical resolution model end to end
//! through the document (PDF) path and the shared conversion API, without
//! any hardware. They never assert physical dimensional accuracy; that is
//! established only by physical measurement (docs/calibration.md).

use letratag_printer::document;
use letratag_printer::geometry::{self, CalibrationProfile, DEFAULT_CALIBRATION};
use letratag_protocol::{Job, Profile};
use std::io::Write;
use std::os::unix::fs::PermissionsExt;

const PROFILE: CalibrationProfile = DEFAULT_CALIBRATION;

fn write_pdf(
    dir: &std::path::Path,
    name: &str,
    width_mm: f64,
    height_mm: f64,
) -> std::path::PathBuf {
    // A centered 40%-of-length x 1 mm ink rectangle, inside the printable band.
    let width_pt = width_mm * 72.0 / 25.4;
    let height_pt = height_mm * 72.0 / 25.4;
    let margin = width_pt * 0.1;
    let rect = format!(
        "{} {} {} {} re f",
        margin,
        height_pt * 0.45,
        width_pt - 2.0 * margin,
        height_pt * 0.1
    );
    let stream = format!("0 g {rect}\n");
    let page = format!(
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {width_pt} {height_pt}] /Resources << >> /Contents 4 0 R >>"
    );
    let objects = [
        "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_string(),
        page,
        format!(
            "<< /Length {} >>\nstream\n{}endstream",
            stream.len(),
            stream
        ),
    ];
    let mut pdf = b"%PDF-1.4\n".to_vec();
    let mut offsets = vec![0];
    for (i, o) in objects.iter().enumerate() {
        offsets.push(pdf.len());
        write!(pdf, "{} 0 obj\n{}\nendobj\n", i + 1, o).unwrap();
    }
    let xref = pdf.len();
    write!(pdf, "xref\n0 5\n0000000000 65535 f \n").unwrap();
    for o in &offsets[1..] {
        writeln!(pdf, "{o:010} 00000 n ").unwrap();
    }
    write!(
        pdf,
        "trailer\n<< /Size 5 /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n"
    )
    .unwrap();
    let path = dir.join(name);
    std::fs::write(&path, pdf).unwrap();
    path
}

fn tempdir() -> tempfile::TempDir {
    tempfile::Builder::new()
        .permissions(std::fs::Permissions::from_mode(0o700))
        .tempdir()
        .unwrap()
}

#[test]
fn feed_lengths_map_to_physical_columns() {
    // 10/20/40/50/100 mm feed lengths at the empirical 315 DPI calibration.
    for (mm, columns) in [
        (10.0, 124usize),
        (20.0, 248),
        (40.0, 496),
        (50.0, 620),
        (100.0, 1240),
    ] {
        assert_eq!(PROFILE.feed_columns(mm), columns, "{mm} mm feed");
        // The same lengths as IPP media (1/100 mm).
        assert_eq!(
            PROFILE.media_length_to_feed_columns((mm * 100.0) as u32),
            columns,
            "{mm} mm media"
        );
    }
}

#[test]
fn pdf_pages_render_at_exactly_the_mapped_columns() {
    // No double-scaling: the rendered raster width is exactly the mapped
    // physical column count for the page length.
    let dir = tempdir();
    for (mm, columns) in [
        (10.0, 124usize),
        (20.0, 248),
        (40.0, 496),
        (50.0, 620),
        (100.0, 1240),
    ] {
        let path = write_pdf(dir.path(), &format!("{mm}mm.pdf"), mm, 12.0);
        let raster = document::pdf(&path).unwrap();
        assert_eq!(raster.width(), columns, "{mm} mm page");
    }
}

#[test]
fn cross_tape_dimensions_and_printable_area() {
    // 12 mm tape at 160 DPI = 76 px; the centered 30-row band is 4.7625 mm,
    // below the tape width (active print rows limit printable height).
    assert_eq!(PROFILE.cross_pixels(12.0), 76);
    assert_eq!(geometry::band_start(76), 23);
    assert!((geometry::printable_band_mm(&PROFILE) - 4.7625).abs() < 1e-9);
    assert!(geometry::printable_band_mm(&PROFILE) < geometry::TAPE_WIDTH_MM);
}

#[test]
fn millimeter_round_trips_and_rounding_bounds() {
    for mm in [10.0, 12.0, 20.0, 40.0, 50.0, 63.5, 100.0, 200.0] {
        let columns = PROFILE.feed_columns(mm);
        let back = columns as f64 * PROFILE.feed_mm_per_column();
        assert!(
            (back - mm).abs() <= PROFILE.feed_mm_per_column() / 2.0 + 1e-9,
            "feed round trip {mm} mm"
        );
        let rows = PROFILE.cross_pixels(mm);
        let back = rows as f64 * PROFILE.cross_mm_per_row();
        assert!(
            (back - mm).abs() <= PROFILE.cross_mm_per_row() / 2.0 + 1e-9,
            "cross round trip {mm} mm"
        );
    }
}

#[test]
fn logical_to_physical_mapping_preserves_proportions() {
    // Mission example: 40 mm x 5 mm logical region at 200 DPI ->
    // logical 315 x 39 px -> physical 496 x 31 px.
    assert_eq!(PROFILE.logical_pixels(40.0), 315);
    assert_eq!(PROFILE.logical_pixels(5.0), 39);
    assert_eq!(PROFILE.feed_columns_for_logical(315), 496);
    assert_eq!(PROFILE.cross_pixels_for_logical(39), 31);
    // A logical square stays a physical square in millimeters.
    for logical_px in [50usize, 100, 200, 315] {
        let feed_mm =
            PROFILE.feed_columns_for_logical(logical_px) as f64 * PROFILE.feed_mm_per_column();
        let cross_mm =
            PROFILE.cross_pixels_for_logical(logical_px) as f64 * PROFILE.cross_mm_per_row();
        let bound = (PROFILE.feed_mm_per_column() + PROFILE.cross_mm_per_row()) / 2.0;
        assert!(
            (feed_mm - cross_mm).abs() <= bound + 1e-9,
            "logical {logical_px} px square"
        );
    }
}

#[test]
fn maximum_dimensions_and_overflow_behavior() {
    // The 200 mm media maximum maps within the protocol raster bound.
    assert_eq!(PROFILE.feed_columns(200.0), 2480);
    assert!(PROFILE.feed_columns_within_protocol(2480));
    assert!(!PROFILE.feed_columns_within_protocol(8001));
    // Pages beyond the media bounds are rejected, not clipped or scaled.
    let dir = tempdir();
    let path = write_pdf(dir.path(), "toolong.pdf", 250.0, 12.0);
    assert!(document::pdf(&path).is_err());
    let path = write_pdf(dir.path(), "tooshort.pdf", 5.0, 12.0);
    assert!(document::pdf(&path).is_err());
}

#[test]
fn protocol_encoding_is_deterministic_for_the_same_raster() {
    // Byte-exact encoding: the same native raster encodes identically twice,
    // and unchanged native-raster fixtures stay byte-exact (the transport
    // crate's payload_preservation tests cover the retained fixtures).
    let dir = tempdir();
    let path = write_pdf(dir.path(), "label.pdf", 50.0, 12.0);
    let raster = document::pdf(&path).unwrap();
    let first = Job::new(&raster, Profile::Legacy).unwrap();
    let second = Job::new(&raster, Profile::Legacy).unwrap();
    assert_eq!(first.body(), second.body());
    assert_eq!(first.writes(244).unwrap(), second.writes(244).unwrap());
    // Feed columns are preserved through encoding: ESC D width == raster width.
    let body = first.body();
    let width = u32::from_le_bytes(body[10..14].try_into().unwrap()) as usize;
    assert_eq!(width, raster.width());
    assert_eq!(body.len(), 24 + 4 * raster.width());
}

#[test]
fn output_geometry_is_consistent_across_document_input_paths() {
    // The same physical 50 mm label through the PDF path and through the
    // physical-raster PNG path produces the same feed width; the shared
    // profile maps millimeters to columns for both.
    let dir = tempdir();
    let path = write_pdf(dir.path(), "label.pdf", 50.0, 12.0);
    let pdf_raster = document::pdf(&path).unwrap();
    assert_eq!(pdf_raster.width(), PROFILE.feed_columns(50.0));
    // A PNG already at the physical geometry renders 1:1 (no rescaling).
    let source = vec![255u8; 620 * 76];
    let png = letratag_render::render_png(
        &blank_png(620, 76),
        letratag_render::RenderOptions {
            width: 620,
            ..Default::default()
        },
    )
    .unwrap();
    let _ = source;
    assert_eq!(png.width(), 620);
}

/// A minimal grayscale PNG of the given size (all white).
fn blank_png(width: u32, height: u32) -> Vec<u8> {
    use image::codecs::png::PngEncoder;
    use image::{ExtendedColorType, ImageEncoder};
    let mut out = Vec::new();
    let encoder = PngEncoder::new(&mut out);
    encoder
        .write_image(
            &vec![255u8; (width * height) as usize],
            width,
            height,
            ExtendedColorType::L8,
        )
        .unwrap();
    out
}

#[test]
fn media_selection_is_deterministic() {
    // Media lengths map to columns through the physical feed pitch only;
    // the logical resolution never enters raster geometry.
    assert_eq!(geometry::MEDIA_DEFAULT_LENGTH, 5000);
    assert_eq!(
        PROFILE.media_length_to_feed_columns(geometry::MEDIA_DEFAULT_LENGTH),
        620
    );
    assert_eq!(
        PROFILE.media_length_to_feed_columns(geometry::MEDIA_MIN_LENGTH),
        124
    );
    assert_eq!(
        PROFILE.media_length_to_feed_columns(geometry::MEDIA_MAX_LENGTH),
        2480
    );
    // A 50 mm page with 50 mm media matches exactly; 49 mm media does not.
    let dir = tempdir();
    let path = write_pdf(dir.path(), "label.pdf", 50.0, 12.0);
    let raster = document::pdf(&path).unwrap();
    let requested = PROFILE.media_length_to_feed_columns(5000);
    assert!(raster.width().abs_diff(requested) <= 1);
    let mismatched = PROFILE.media_length_to_feed_columns(4900);
    assert!(raster.width().abs_diff(mismatched) > 1);
}
