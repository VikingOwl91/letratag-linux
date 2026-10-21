use crate::geometry;
use letratag_protocol::Raster;
use std::io::{Cursor, Read};
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Duration;
use wait_timeout::ChildExt;

pub const MAX_FILE: u64 = 8 * 1024 * 1024;

/// Pixels for a millimeter length at the device resolution.
fn px(mm: f64) -> f64 {
    geometry::mm_to_dots(mm)
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

pub fn pdf(path: &Path) -> Result<Raster, Error> {
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
    let mut checker = Command::new("/usr/bin/prlimit")
        .args([
            "--as=536870912",
            "--cpu=10",
            "--fsize=8388608",
            "--nofile=64",
            "--",
        ])
        .arg("/usr/bin/qpdf")
        .arg("--check")
        .arg(&input)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    match checker.wait_timeout(Duration::from_secs(15))? {
        Some(status) if status.success() => (),
        _ => {
            let _ = checker.kill();
            let _ = checker.wait();
            return Err(Error::Input);
        }
    }
    let mut counter = Command::new("/usr/bin/prlimit")
        .args([
            "--as=536870912",
            "--cpu=10",
            "--fsize=8388608",
            "--nofile=64",
            "--",
        ])
        .arg("/usr/bin/qpdf")
        .arg("--show-npages")
        .arg(&input)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;
    match counter.wait_timeout(Duration::from_secs(15))? {
        Some(status) if status.success() => (),
        _ => {
            let _ = counter.kill();
            let _ = counter.wait();
            return Err(Error::Input);
        }
    }
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
    let prefix = scratch.path().join("page");
    let mut child = Command::new("/usr/bin/prlimit")
        .args([
            "--as=536870912",
            "--cpu=10",
            "--fsize=8388608",
            "--nofile=64",
            "--",
        ])
        .arg("/usr/bin/pdftoppm")
        .args(["-f", "1", "-l", "2"])
        .arg("-r")
        .arg((geometry::RESOLUTION_DPI as u32).to_string())
        .args(["-gray", "-png"])
        .arg(&input)
        .arg(&prefix)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    match child.wait_timeout(Duration::from_secs(15))? {
        Some(status) if status.success() => (),
        _ => {
            let _ = child.kill();
            let _ = child.wait();
            return Err(Error::Conversion);
        }
    }
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
    // One 12 mm page, at most 200 mm long, rendered at the device
    // resolution. The 12 mm side allows one pixel of rounding at either
    // end.
    let page = px(geometry::TAPE_WIDTH_MM);
    let page_lo = page.floor() as usize;
    let page_hi = page.ceil() as usize;
    let max_len = px(geometry::MAX_LENGTH_MM).ceil() as usize;
    let landscape = (page_lo..=page_hi).contains(&h) && (1..=max_len).contains(&w);
    let portrait = (page_lo..=page_hi).contains(&w) && (1..=max_len).contains(&h);
    if !landscape && !portrait {
        return Err(Error::Geometry);
    }
    let image = image::load_from_memory_with_format(&png, image::ImageFormat::Png)
        .map_err(|_| Error::Conversion)?
        .into_luma8();
    let (feed, cross) = if landscape { (w, h) } else { (h, w) };
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
