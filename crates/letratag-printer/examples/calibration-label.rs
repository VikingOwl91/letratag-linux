//! Calibration label diagnostic: known physical reference lengths.
//!
//! Generates a 110 x 12 mm label PDF with one horizontal reference line per
//! feed length (10, 20, 40, 50, 100 mm), each exactly `feed_columns(L)`
//! columns at the configured calibration profile, plus end ticks and text
//! labels. The commanded columns and expected physical lengths are printed
//! for every reference, so a printed label can be measured and the profile
//! fitted (docs/calibration.md).
//!
//! Modes:
//!   --output <file.pdf>   write the calibration label PDF (default)
//!   --check               generate, render through the document path and
//!                         verify the raster geometry offline (no printing)
//!   --calibration feed=<dpi>,cross=<dpi>
//!
//! This example never prints by itself; submit the PDF through the
//! recommended Direct IPP path with explicit operator authorization.

use letratag_printer::document;
use letratag_printer::geometry::{self, CalibrationProfile};

/// Reference feed lengths in millimeters, with band rows (wire rows).
const REFERENCES: &[(f64, usize)] = &[(10.0, 3), (20.0, 9), (40.0, 15), (50.0, 21), (100.0, 27)];

/// Page: 110 mm feed x 12 mm tape, with a 10 mm leading margin.
const PAGE_MM: f64 = 110.0;
const TAPE_MM: f64 = 12.0;
const MARGIN_MM: f64 = 10.0;

fn build_pdf(profile: &CalibrationProfile) -> (Vec<u8>, Vec<(f64, usize, usize)>) {
    let page_pt = PAGE_MM * 72.0 / 25.4;
    let tape_pt = TAPE_MM * 72.0 / 25.4;
    let margin_pt = MARGIN_MM * 72.0 / 25.4;
    let cross_px = profile.cross_pixels(TAPE_MM);
    let band_start = geometry::band_start(cross_px);
    let mut rects = Vec::new();
    let mut commanded = Vec::new();
    for &(mm, wire_row) in REFERENCES {
        let columns = profile.feed_columns(mm);
        commanded.push((mm, columns, wire_row));
        // Wire rows are 1-based within the printable band; the wire row maps
        // to a page row (from the top) via band_start. PDF y is measured
        // from the bottom of the page; the rect's y is its bottom edge, so
        // center the 0.45 pt line on the target row.
        let page_row_top = band_start + wire_row - 1;
        let y_pt = tape_pt - (page_row_top as f64 + 1.0) * tape_pt / cross_px as f64;
        let x0 = margin_pt;
        let x1 = margin_pt + mm * 72.0 / 25.4;
        // Reference line, 0.45 pt thick, with end ticks. No text: the
        // printable band is only 30 rows, and the printed table identifies
        // each reference by its wire row.
        rects.push(format!("0 g {x0:.4} {y_pt:.4} {:.4} 0.45 re f", x1 - x0));
        rects.push(format!(
            "0 g {:.4} {:.4} 0.45 1.6 re f",
            x0 - 0.225,
            y_pt - 0.8
        ));
        rects.push(format!(
            "0 g {:.4} {:.4} 0.45 1.6 re f",
            x1 - 0.225,
            y_pt - 0.8
        ));
    }
    let stream = rects.join("\n");
    let page = format!(
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {page_pt:.6} {tape_pt:.6}] /Resources << /Font << /F1 5 0 R >> >> /Contents 4 0 R >>"
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
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_string(),
    ];
    let mut pdf = b"%PDF-1.4\n".to_vec();
    let mut offsets = vec![0];
    for (i, o) in objects.iter().enumerate() {
        offsets.push(pdf.len());
        pdf.extend_from_slice(format!("{} 0 obj\n{}\nendobj\n", i + 1, o).as_bytes());
    }
    let xref = pdf.len();
    pdf.extend_from_slice(b"xref\n0 6\n0000000000 65535 f \n");
    for o in &offsets[1..] {
        pdf.extend_from_slice(format!("{o:010} 00000 n \n").as_bytes());
    }
    pdf.extend_from_slice(
        format!("trailer\n<< /Size 6 /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n").as_bytes(),
    );
    (pdf, commanded)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut output: Option<String> = None;
    let mut check = false;
    let mut calibration: Option<String> = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--output" => output = Some(args.next().ok_or("--output requires a value")?),
            "--check" => check = true,
            "--calibration" => {
                calibration = Some(args.next().ok_or("--calibration requires a value")?)
            }
            other => return Err(format!("unknown argument: {other}").into()),
        }
    }
    let profile = match calibration {
        Some(text) => geometry::parse_calibration_override(&text)?,
        None => geometry::DEFAULT_CALIBRATION,
    };
    let (pdf, commanded) = build_pdf(&profile);

    println!(
        "calibration profile: feed {:.1} dpi, cross {:.1} dpi (empirical estimates)",
        profile.feed_dpi, profile.cross_dpi
    );
    println!(
        "page: {PAGE_MM} mm x {TAPE_MM} mm; printable band {} rows = {:.4} mm",
        geometry::PRINTABLE_ROWS,
        geometry::printable_band_mm(&profile)
    );
    println!(
        "{:>8} {:>12} {:>14} {:>12}",
        "length", "columns", "expected mm", "mm/column"
    );
    for (mm, columns, _) in &commanded {
        println!(
            "{:>8.0} {:>12} {:>14.4} {:>12.6}",
            mm,
            columns,
            *columns as f64 * profile.feed_mm_per_column(),
            profile.feed_mm_per_column()
        );
    }

    if check {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("calibration.pdf");
        std::fs::write(&path, &pdf)?;
        let raster = document::pdf_with_calibration(&path, &profile)?;
        let expected_width = profile.feed_columns(PAGE_MM);
        assert_eq!(
            raster.width(),
            expected_width,
            "raster width == mapped columns"
        );
        // Every reference line: ink extent matches the commanded columns
        // (antialiasing tolerance of 2 columns at each end). The raster is
        // the wire frame, so each line sits at its wire row directly.
        for &(mm, columns, wire_row) in &commanded {
            let xs: Vec<usize> = (0..raster.width())
                .filter(|&x| raster.get(x, wire_row))
                .collect();
            assert!(!xs.is_empty(), "reference line {mm} mm missing");
            let extent = xs.len();
            assert!(
                (extent as i64 - columns as i64).abs() <= 2,
                "reference line {mm} mm: ink {extent} columns, commanded {columns}"
            );
        }
        println!(
            "CHECK PASS: raster {expected_width} columns; 5 reference lines at commanded columns"
        );
        return Ok(());
    }

    let path = output.unwrap_or_else(|| "calibration-label.pdf".to_string());
    std::fs::write(&path, &pdf)?;
    println!("wrote {path} ({} bytes)", pdf.len());
    println!(
        "submit through Direct IPP with explicit media (110 mm x 12 mm) only with operator authorization"
    );
    Ok(())
}
