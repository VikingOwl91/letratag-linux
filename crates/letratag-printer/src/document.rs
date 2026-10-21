use crate::geometry::{self, CalibrationProfile};
use letratag_protocol::Raster;
use std::io::{Cursor, Read};
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Duration;
use wait_timeout::ChildExt;

pub const MAX_FILE: u64 = 8 * 1024 * 1024;

/// Resource-limited command runner shared by the document pipeline.
fn limited(program: &str, args: &[&str]) -> Command {
    let mut command = Command::new("/usr/bin/prlimit");
    command
        .args([
            "--as=536870912",
            "--cpu=10",
            "--fsize=8388608",
            "--nofile=64",
            "--",
        ])
        .arg(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    command
}

fn run_status(child: &mut std::process::Child) -> Result<(), std::io::Error> {
    match child.wait_timeout(Duration::from_secs(15))? {
        Some(status) if status.success() => Ok(()),
        _ => {
            let _ = child.kill();
            let _ = child.wait();
            Err(std::io::Error::other("command failed or timed out"))
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("input file is not a bounded regular file")]
    Input,
    #[error("PDF conversion failed or exceeded its resource limits")]
    Conversion,
    #[error(
        "PDF requires exactly one 12-mm page, at most 200 mm long, with ink inside the centered 30-row printable area"
    )]
    Geometry,

    #[error(
        "PDF page rotation 90/270 is not supported (anisotropic rendering distorts rotated pages); export the page pre-rotated"
    )]
    Rotated,

    #[error("document I/O failed")]
    Io(#[from] std::io::Error),
}

pub fn bounded_read(path: &Path) -> Result<Vec<u8>, Error> {
    let file = std::fs::File::open(path)?;
    let md = file.metadata()?;
    if !md.is_file() || md.len() > MAX_FILE {
        return Err(Error::Input);
    }
    let mut data = Vec::new();
    file.take(MAX_FILE + 1).read_to_end(&mut data)?;
    if data.len() as u64 > MAX_FILE {
        return Err(Error::Input);
    }
    Ok(data)
}

/// Render a one-page PDF with the default calibration profile.
pub fn pdf(path: &Path) -> Result<Raster, Error> {
    pdf_with_calibration(path, &geometry::DEFAULT_CALIBRATION)
}

/// The page geometry of a PDF, from `pdfinfo`.
#[derive(Debug, Clone, Copy, PartialEq)]
struct PageGeometry {
    /// Page width in millimeters (the page's own x axis, before rotation).
    width_mm: f64,
    /// Page height in millimeters (the page's own y axis, before rotation).
    height_mm: f64,
    /// Page rotation in degrees (0/90/180/270).
    rotation: u32,
}

fn read_page_geometry(input: &Path) -> Result<PageGeometry, Error> {
    let mut child = limited("/usr/bin/pdfinfo", &["-f", "1", "-l", "1"]);
    let output = child
        .arg(input)
        .stdout(Stdio::piped())
        .spawn()?
        .wait_with_output()
        .map_err(|_| Error::Conversion)?;
    if !output.status.success() {
        return Err(Error::Conversion);
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let mut size: Option<(f64, f64)> = None;
    let mut rotation: Option<u32> = None;
    for line in text.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("Page") {
            let rest = rest.trim_start();
            if let Some(rest) = rest.strip_prefix("1") {
                let rest = rest.trim_start();
                if let Some(rest) = rest.strip_prefix("size:") {
                    let rest = rest.trim();
                    let mut parts = rest.split_whitespace();
                    let width: f64 = parts
                        .next()
                        .and_then(|v| v.parse().ok())
                        .ok_or(Error::Conversion)?;
                    let x = parts.next().ok_or(Error::Conversion)?;
                    let height: f64 = parts
                        .next()
                        .and_then(|v| v.parse().ok())
                        .ok_or(Error::Conversion)?;
                    if x != "x" {
                        return Err(Error::Conversion);
                    }
                    size = Some((width, height));
                } else if let Some(rest) = rest.strip_prefix("rot:") {
                    rotation = Some(rest.trim().parse().map_err(|_| Error::Conversion)?);
                }
            }
        }
    }
    let (width_pt, height_pt) = size.ok_or(Error::Conversion)?;
    Ok(PageGeometry {
        width_mm: geometry::pt_to_mm(width_pt),
        height_mm: geometry::pt_to_mm(height_pt),
        rotation: rotation.ok_or(Error::Conversion)? % 360,
    })
}

/// Render a one-page PDF to the physical raster described by `profile`.
///
/// The PDF is rendered directly at the target physical raster resolution
/// (`pdftoppm -scale-to-x/-scale-to-y`, exact anisotropic scaling), so the
/// resolution conversion happens once, at this rendering boundary, and no
/// rasterized content is resampled. Physical proportions are preserved: a
/// region of W x H millimeters renders to `feed_columns(W)` x
/// `cross_pixels(H)` physical pixels (swapping axes for portrait pages) and
/// prints at W x H millimeters.
pub fn pdf_with_calibration(path: &Path, profile: &CalibrationProfile) -> Result<Raster, Error> {
    let data = bounded_read(path)?;
    if !data.starts_with(b"%PDF-") {
        return Err(Error::Input);
    }
    let trimmed = data.trim_ascii_end();
    if !trimmed.ends_with(b"%%EOF") {
        return Err(Error::Input);
    }
    let scratch = tempfile::Builder::new()
        .permissions(std::fs::Permissions::from_mode(0o700))
        .tempdir()?;
    // Private copy makes path interpretation and input mutation independent of callers.
    let input = scratch.path().join("input.pdf");
    std::fs::write(&input, data)?;
    // Reject damaged PDFs instead of relying on Poppler's best-effort recovery.
    let mut checker = limited("/usr/bin/qpdf", &["--check"]);
    let mut checker = checker.arg(&input).spawn()?;
    run_status(&mut checker).map_err(|_| Error::Input)?;
    let mut counter = limited("/usr/bin/qpdf", &["--show-npages"]);
    let mut counter = counter.arg(&input).stdout(Stdio::piped()).spawn()?;
    let mut pages = Vec::new();
    counter
        .stdout
        .take()
        .ok_or(Error::Input)?
        .take(32)
        .read_to_end(&mut pages)?;
    if pages.trim_ascii() != b"1" {
        return Err(Error::Geometry);
    }
    let page = read_page_geometry(&input)?;

    // The page must have exactly one ~12 mm side (the tape width); the other
    // side is the label length within the media bounds. Tolerances are half
    // a physical pixel so rounding at either end is accepted.
    let cross_tol = profile.cross_mm_per_row() / 2.0;
    let feed_tol = profile.feed_mm_per_column() / 2.0;
    let tape = geometry::TAPE_WIDTH_MM;
    let width_is_tape = (page.width_mm - tape).abs() <= cross_tol;
    let height_is_tape = (page.height_mm - tape).abs() <= cross_tol;
    let (authored_landscape, feed_mm, cross_mm) = match (width_is_tape, height_is_tape) {
        (false, true) => (true, page.width_mm, page.height_mm),
        (true, false) => (false, page.height_mm, page.width_mm),
        _ => return Err(Error::Geometry),
    };
    if !(geometry::MIN_LENGTH_MM - feed_tol..=geometry::MAX_LENGTH_MM + feed_tol).contains(&feed_mm)
    {
        return Err(Error::Geometry);
    }
    let feed_columns = profile.feed_columns(feed_mm);
    let cross_pixels = profile.cross_pixels(cross_mm);
    if !profile.feed_columns_within_protocol(feed_columns) {
        return Err(Error::Geometry);
    }
    // pdftoppm -scale-to-x/-scale-to-y apply to the page's own axes; a
    // portrait page therefore renders with the cross scale on its width.
    let (scale_x, scale_y) = if authored_landscape {
        (feed_columns, cross_pixels)
    } else {
        (cross_pixels, feed_columns)
    };
    // A 90/270-degree page rotation swaps the rendered output axes, and
    // Poppler's anisotropic scaling distorts content on rotated pages
    // (verified 2026-10-21: a 40 mm reference rendered at ~20 mm). Reject
    // rotated pages with an actionable error instead of distorting them.
    // 0/180 rotations preserve the axes and render correctly.
    if page.rotation % 180 == 90 {
        return Err(Error::Rotated);
    }
    let (expected_w, expected_h) = (scale_x, scale_y);

    let prefix = scratch.path().join("page");
    let mut child = limited(
        "/usr/bin/pdftoppm",
        &[
            "-f",
            "1",
            "-l",
            "2",
            "-scale-to-x",
            &scale_x.to_string(),
            "-scale-to-y",
            &scale_y.to_string(),
            "-gray",
            "-png",
        ],
    );
    let mut child = child.arg(&input).arg(&prefix).spawn()?;
    run_status(&mut child).map_err(|_| Error::Conversion)?;
    let mut pages = Vec::new();
    for entry in std::fs::read_dir(scratch.path())? {
        let p = entry?.path();
        if p.extension().is_some_and(|e| e == "png") {
            pages.push(p);
        }
    }
    if pages.len() != 1 {
        return Err(Error::Geometry);
    }
    let png = bounded_read(&pages[0])?;
    let dims = image::ImageReader::new(Cursor::new(&png))
        .with_guessed_format()
        .map_err(|_| Error::Conversion)?
        .into_dimensions()
        .map_err(|_| Error::Conversion)?;
    let (w, h) = (dims.0 as usize, dims.1 as usize);
    // The render must be exactly the computed physical raster: one
    // conversion, no resampling, no implicit fitting.
    if (w, h) != (expected_w, expected_h) {
        return Err(Error::Conversion);
    }
    // Orientation of the rendered output: the feed axis is the long side
    // (feed columns always exceed the ~76-pixel tape width).
    let landscape = w >= h;
    let (feed, cross) = if landscape { (w, h) } else { (h, w) };
    if feed != feed_columns || cross != cross_pixels {
        return Err(Error::Geometry);
    }
    let image = image::load_from_memory_with_format(&png, image::ImageFormat::Png)
        .map_err(|_| Error::Conversion)?
        .into_luma8();
    // The printable band: PRINTABLE_ROWS centered in the rendered page,
    // mapped 1:1 onto the centered content rows of the 32-row wire frame.
    let start = geometry::band_start(cross);
    let mut out = vec![0; feed * geometry::FRAME_ROWS];
    for x in 0..feed {
        for y in 0..cross {
            let (ix, iy) = if landscape {
                (x, y)
            } else {
                (cross - 1 - y, x)
            };
            if image.get_pixel(ix as u32, iy as u32)[0] >= 128 {
                continue;
            }
            if !(start..start + geometry::PRINTABLE_ROWS).contains(&y) {
                return Err(Error::Geometry);
            }
            out[geometry::wire_row(y - start) * feed + x] = 1;
        }
    }
    Raster::new(feed, out).map_err(|_| Error::Geometry)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write_pdf(
        dir: &Path,
        name: &str,
        width_pt: f64,
        height_pt: f64,
        rotation: u32,
        rect: &str,
    ) -> std::path::PathBuf {
        let mut pdf = b"%PDF-1.4\n".to_vec();
        let stream = format!("0 g {rect}\n");
        let page = if rotation == 0 {
            format!(
                "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {width_pt} {height_pt}] /Resources << >> /Contents 4 0 R >>"
            )
        } else {
            format!(
                "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {width_pt} {height_pt}] /Rotate {rotation} /Resources << >> /Contents 4 0 R >>"
            )
        };
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
    fn landscape_page_renders_at_physical_resolution() {
        // 50 x 12 mm page with a centered 40 mm x 4 mm ink rectangle
        // (page rows 16..19 pt of 34.016 pt, i.e. inside the 30-row band).
        let dir = tempdir();
        let path = write_pdf(
            dir.path(),
            "label.pdf",
            141.732283,
            34.015748,
            0,
            "4.5 16.008 113.4 1.6 re f",
        );
        let raster = pdf(&path).unwrap();
        // 50 mm at 315 DPI = 620 feed columns; 12 mm at 160 DPI = 76 px page.
        assert_eq!(raster.width(), 620);
        // The 40 mm x 4 mm ink rectangle: 40 mm at 315 DPI = 496 columns.
        // Edges are antialiased by Poppler, so allow a couple of columns of
        // tolerance at each end; the raster geometry itself is exact.
        let xs: Vec<usize> = (0..raster.width())
            .filter(|&x| (0..32).any(|y| raster.get(x, y)))
            .collect();
        assert_eq!(*xs.first().unwrap(), 20);
        assert!((514..=517).contains(xs.last().unwrap()));
        assert!((494..=498).contains(&xs.len()));
        // Ink is inside the printable band (wire rows 1..=30).
        let ys: Vec<usize> = (0..32)
            .filter(|&y| (0..raster.width()).any(|x| raster.get(x, y)))
            .collect();
        assert!(ys.iter().all(|y| (1..=30).contains(y)));
    }

    #[test]
    fn portrait_page_renders_at_physical_resolution_with_rotation() {
        // 12 x 50 mm portrait page; same rectangle, authored vertically.
        let dir = tempdir();
        let path = write_pdf(
            dir.path(),
            "portrait.pdf",
            34.015748,
            141.732283,
            0,
            "16.008 4.5 1.6 113.4 re f",
        );
        let raster = pdf(&path).unwrap();
        assert_eq!(raster.width(), 620);
        let xs: Vec<usize> = (0..raster.width())
            .filter(|&x| (0..32).any(|y| raster.get(x, y)))
            .collect();
        assert_eq!(xs.len(), 496);
    }

    #[test]
    fn rotated_pages_are_rejected_not_distorted() {
        // /Rotate 90/270 pages are rejected with an actionable error: Poppler's
        // anisotropic scaling distorts content on rotated pages (verified
        // 2026-10-21: a 40 mm reference rendered at ~20 mm). /Rotate 180
        // preserves the axes and renders correctly.
        let dir = tempdir();
        let path = write_pdf(
            dir.path(),
            "rotated90.pdf",
            141.732283,
            34.015748,
            90,
            "4.5 16.008 113.4 1.6 re f",
        );
        assert!(matches!(pdf(&path), Err(Error::Rotated)));
        let path = write_pdf(
            dir.path(),
            "rotated270.pdf",
            141.732283,
            34.015748,
            270,
            "4.5 16.008 113.4 1.6 re f",
        );
        assert!(matches!(pdf(&path), Err(Error::Rotated)));
        let path = write_pdf(
            dir.path(),
            "rotated180.pdf",
            141.732283,
            34.015748,
            180,
            "4.5 16.008 113.4 1.6 re f",
        );
        let raster = pdf(&path).unwrap();
        assert_eq!(raster.width(), 620);
        let xs: Vec<usize> = (0..raster.width())
            .filter(|&x| (0..32).any(|y| raster.get(x, y)))
            .collect();
        // 180-degree rotation flips the rectangle horizontally.
        assert_eq!(*xs.first().unwrap(), 620 - 516);
        assert!((494..=498).contains(&xs.len()));
    }

    #[test]
    fn ink_outside_the_printable_band_is_rejected_not_cropped() {
        // Rectangle at the top edge of the page: outside the centered band.
        let dir = tempdir();
        let path = write_pdf(
            dir.path(),
            "outside.pdf",
            141.732283,
            34.015748,
            0,
            "4.5 1.0 113.4 1.6 re f",
        );
        assert!(matches!(pdf(&path), Err(Error::Geometry)));
    }

    #[test]
    fn wrong_page_width_is_rejected() {
        // 11 mm tape side: not a 12 mm page.
        let dir = tempdir();
        let path = write_pdf(
            dir.path(),
            "narrow.pdf",
            141.732283,
            31.1811,
            0,
            "4.5 16 1 1 re f",
        );
        assert!(matches!(pdf(&path), Err(Error::Geometry)));
        // 250 mm feed length: beyond the 200 mm media maximum.
        let path = write_pdf(
            dir.path(),
            "too-long.pdf",
            708.661417,
            34.015748,
            0,
            "4.5 16 1 1 re f",
        );
        assert!(matches!(pdf(&path), Err(Error::Geometry)));
        // Square page: ambiguous orientation.
        let path = write_pdf(
            dir.path(),
            "square.pdf",
            34.015748,
            34.015748,
            0,
            "4.5 16 1 1 re f",
        );
        assert!(matches!(pdf(&path), Err(Error::Geometry)));
    }

    #[test]
    fn calibration_override_changes_the_physical_raster() {
        let dir = tempdir();
        let path = write_pdf(
            dir.path(),
            "label.pdf",
            141.732283,
            34.015748,
            0,
            "4.5 16.008 113.4 1.6 re f",
        );
        let profile = CalibrationProfile {
            feed_dpi: 160.0,
            cross_dpi: 160.0,
        };
        let raster = pdf_with_calibration(&path, &profile).unwrap();
        // 50 mm at 160 DPI = 315 columns (the legacy square model).
        assert_eq!(raster.width(), 315);
        let custom = CalibrationProfile {
            feed_dpi: 314.0,
            cross_dpi: 160.0,
        };
        let raster = pdf_with_calibration(&path, &custom).unwrap();
        // 50 mm at 314 DPI = 618.11 -> 618 columns.
        assert_eq!(raster.width(), 618);
    }

    #[test]
    fn monochrome_threshold_is_applied() {
        // Mid-gray (64) is ink; light gray (200) is not.
        let dir = tempdir();
        let path = write_pdf(
            dir.path(),
            "gray.pdf",
            141.732283,
            34.015748,
            0,
            "0.25 g 4.5 16.008 113.4 1.6 re f",
        );
        let raster = pdf(&path).unwrap();
        assert!((0..raster.width()).any(|x| (0..32).any(|y| raster.get(x, y))));
    }
}
