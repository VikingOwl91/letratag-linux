//! Deterministic image rendering. The canvas is feed columns by 32 head rows.
//!
//! Active rows are a design choice, not a physical printhead calibration. Images
//! keep their aspect ratio (rounded down to whole pixels, with a one-pixel
//! minimum), including any existing white margins, and are centered on the full
//! canvas. An odd spare pixel goes on the right or bottom. Scaling uses nearest
//! neighbors sampled at pixel centers. No text, font, or PDF rendering is done.
#![doc = include_str!("../README.md")]

use std::io::Cursor;

use image::{
    DynamicImage, ExtendedColorType, GenericImageView, ImageDecoder, ImageEncoder, Limits,
    codecs::png::{PngDecoder, PngEncoder},
};
use letratag_protocol::{HEAD_ROWS, MAX_COLUMNS, Raster};

pub const MAX_PNG_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_SOURCE_DIMENSION: usize = 8192;
pub const MAX_SOURCE_PIXELS: usize = 16_000_000;
pub const MAX_DECODE_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RenderOptions {
    pub width: usize,
    pub active_rows: usize,
    /// Luminance strictly below this value becomes black.
    pub threshold: u8,
    /// Apply a deterministic 4x4 Bayer threshold offset, anchored to the canvas.
    pub dither: bool,
}

impl Default for RenderOptions {
    fn default() -> Self {
        Self {
            width: 400,
            active_rows: 30,
            threshold: 128,
            dither: false,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("output width {width} is outside 1..={MAX_COLUMNS}")]
    InvalidOutputWidth { width: usize },
    #[error("active row count {active_rows} is outside 1..={HEAD_ROWS}")]
    InvalidActiveRows { active_rows: usize },
    #[error("source dimensions {width}x{height} are outside 1..={MAX_SOURCE_DIMENSION}")]
    InvalidSourceDimensions { width: usize, height: usize },
    #[error("source has {pixels} pixels, exceeding {MAX_SOURCE_PIXELS}")]
    TooManyPixels { pixels: usize },
    #[error("grayscale data has {actual} bytes, expected {expected}")]
    GrayscaleLength { expected: usize, actual: usize },
    #[error("PNG has {bytes} bytes, exceeding {MAX_PNG_BYTES}")]
    FileTooLarge { bytes: usize },
    #[error("missing or invalid PNG signature or IHDR header")]
    InvalidPngHeader,
    #[error("decoded image needs {bytes} bytes, exceeding {MAX_DECODE_BYTES}")]
    AllocationLimit { bytes: u64 },
    #[error("image size arithmetic overflow")]
    ArithmeticOverflow,
    #[error(transparent)]
    Image(#[from] image::ImageError),
    #[error("invalid protocol raster: {0}")]
    Raster(#[source] letratag_protocol::Error),
}

/// Decode only PNG. Header dimensions are checked before decoder construction;
/// `Limits` bounds its buffers and decoded storage is checked before allocation.
/// Alpha is composited on white before integer Rec. 709 luminance conversion.
/// For APNG, only the default PNG image is rendered, not animation frames.
pub fn render_png(data: &[u8], options: RenderOptions) -> Result<Raster, Error> {
    validate_options(options)?;
    if data.len() > MAX_PNG_BYTES {
        return Err(Error::FileTooLarge { bytes: data.len() });
    }
    let (width, height) = validate_png_header(data)?;
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_SOURCE_DIMENSION as u32);
    limits.max_image_height = Some(MAX_SOURCE_DIMENSION as u32);
    limits.max_alloc = Some(MAX_DECODE_BYTES);
    let mut decoder = PngDecoder::with_limits(Cursor::new(data), limits.clone())?;
    let bytes = decoder.total_bytes();
    if bytes > MAX_DECODE_BYTES {
        return Err(Error::AllocationLimit { bytes });
    }
    // Reserve the decoded buffer as ImageReader does, leaving the remaining
    // budget for decoder work. Construction already had its own bounded limit.
    limits.reserve(bytes)?;
    decoder.set_limits(limits)?;
    let image = DynamicImage::from_decoder(decoder)?;
    render_samples(width, height, options, |x, y| {
        let [r, g, b, a] = image.get_pixel(x as u32, y as u32).0;
        let composite =
            |c: u8| (u32::from(c) * u32::from(a) + 255 * (255 - u32::from(a)) + 127) / 255;
        ((2126 * composite(r) + 7152 * composite(g) + 722 * composite(b) + 5000) / 10_000) as u8
    })
}

/// Render row-major 8-bit grayscale: zero is black and 255 is white.
pub fn render_gray(
    width: usize,
    height: usize,
    pixels: &[u8],
    options: RenderOptions,
) -> Result<Raster, Error> {
    validate_options(options)?;
    let expected = validate_source(width, height)?;
    if pixels.len() != expected {
        return Err(Error::GrayscaleLength {
            expected,
            actual: pixels.len(),
        });
    }
    render_samples(width, height, options, |x, y| pixels[y * width + x])
}

/// Encode the actual full-height raster, without rescaling or trimming.
pub fn preview_png(raster: &Raster) -> Result<Vec<u8>, Error> {
    let pixels: Vec<u8> = raster
        .pixels()
        .iter()
        .map(|&pixel| if pixel == 0 { 255 } else { 0 })
        .collect();
    let mut output = Vec::new();
    PngEncoder::new(&mut output).write_image(
        &pixels,
        raster.width() as u32,
        HEAD_ROWS as u32,
        ExtendedColorType::L8,
    )?;
    Ok(output)
}

fn validate_options(options: RenderOptions) -> Result<(), Error> {
    if !(1..=MAX_COLUMNS).contains(&options.width) {
        return Err(Error::InvalidOutputWidth {
            width: options.width,
        });
    }
    if !(1..=HEAD_ROWS).contains(&options.active_rows) {
        return Err(Error::InvalidActiveRows {
            active_rows: options.active_rows,
        });
    }
    Ok(())
}

fn validate_source(width: usize, height: usize) -> Result<usize, Error> {
    if !(1..=MAX_SOURCE_DIMENSION).contains(&width) || !(1..=MAX_SOURCE_DIMENSION).contains(&height)
    {
        return Err(Error::InvalidSourceDimensions { width, height });
    }
    let pixels = width.checked_mul(height).ok_or(Error::ArithmeticOverflow)?;
    if pixels > MAX_SOURCE_PIXELS {
        return Err(Error::TooManyPixels { pixels });
    }
    Ok(pixels)
}

fn validate_png_header(data: &[u8]) -> Result<(usize, usize), Error> {
    if data.len() < 33
        || &data[..8] != b"\x89PNG\r\n\x1a\n"
        || data[8..12] != 13_u32.to_be_bytes()
        || &data[12..16] != b"IHDR"
    {
        return Err(Error::InvalidPngHeader);
    }
    let width = u32::from_be_bytes(data[16..20].try_into().unwrap()) as usize;
    let height = u32::from_be_bytes(data[20..24].try_into().unwrap()) as usize;
    let pixels = validate_source(width, height)?;
    // Sub-byte palette/grayscale data expands to 8 bits. PNG palette+tRNS can
    // expand to RGBA, so account for four channels even before reading tRNS.
    let channels = match data[25] {
        0 => 1_u64,
        2 => 3,
        3 | 6 => 4,
        4 => 2,
        _ => return Err(Error::InvalidPngHeader),
    };
    let sample_bytes = match data[24] {
        1 | 2 | 4 | 8 => 1,
        16 => 2,
        _ => return Err(Error::InvalidPngHeader),
    };
    let bytes = (pixels as u64)
        .checked_mul(channels)
        .and_then(|bytes| bytes.checked_mul(sample_bytes))
        .ok_or(Error::ArithmeticOverflow)?;
    if bytes > MAX_DECODE_BYTES {
        return Err(Error::AllocationLimit { bytes });
    }
    Ok((width, height))
}

fn render_samples(
    source_width: usize,
    source_height: usize,
    options: RenderOptions,
    sample: impl Fn(usize, usize) -> u8,
) -> Result<Raster, Error> {
    let width_fit = source_width
        .checked_mul(options.active_rows)
        .ok_or(Error::ArithmeticOverflow)?;
    let height_fit = source_height
        .checked_mul(options.width)
        .ok_or(Error::ArithmeticOverflow)?;
    let (scaled_width, scaled_height) = if width_fit >= height_fit {
        (options.width, (height_fit / source_width).max(1))
    } else {
        ((width_fit / source_height).max(1), options.active_rows)
    };
    let offset_x = (options.width - scaled_width) / 2;
    let offset_y = (HEAD_ROWS - scaled_height) / 2;
    let length = options
        .width
        .checked_mul(HEAD_ROWS)
        .ok_or(Error::ArithmeticOverflow)?;
    let mut pixels = vec![0; length];
    // Validated dimensions bound the following indexing and center-sampling
    // products below 2 * MAX_COLUMNS * MAX_SOURCE_DIMENSION, even on 32-bit.
    for y in 0..scaled_height {
        let source_y = (2 * y + 1) * source_height / (2 * scaled_height);
        for x in 0..scaled_width {
            let source_x = (2 * x + 1) * source_width / (2 * scaled_width);
            let canvas_x = x + offset_x;
            let canvas_y = y + offset_y;
            let threshold = if options.dither {
                const BAYER: [[i16; 4]; 4] =
                    [[0, 8, 2, 10], [12, 4, 14, 6], [3, 11, 1, 9], [15, 7, 13, 5]];
                (i16::from(options.threshold) + BAYER[canvas_y % 4][canvas_x % 4] * 16 + 8 - 128)
                    .clamp(0, 255) as u8
            } else {
                options.threshold
            };
            pixels[canvas_y * options.width + canvas_x] =
                u8::from(sample(source_x, source_y) < threshold);
        }
    }
    Raster::new(options.width, pixels).map_err(Error::Raster)
}
