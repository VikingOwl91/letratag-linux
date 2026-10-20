use image::{ExtendedColorType, ImageEncoder, codecs::png::PngEncoder};
use letratag_protocol::{HEAD_ROWS, MAX_COLUMNS, Raster};
use letratag_render::{
    Error, MAX_DECODE_BYTES, MAX_PNG_BYTES, MAX_SOURCE_PIXELS, RenderOptions, preview_png,
    render_gray, render_png,
};
use proptest::prelude::*;

// Fixed 2x2 RGBA PNG: opaque black, opaque white, transparent black, half black.
// This is a checked-in fixture, not an encoder round-trip used as input evidence.
const RGBA_PNG_HEX: &str = "89504e470d0a1a0a0000000d494844520000000200000002080600000072b60d240000001249444154789c63606060f80f020c50d000003c56057cc09992930000000049454e44ae426082";

fn fixture() -> Vec<u8> {
    RGBA_PNG_HEX
        .as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect()
}

fn options(width: usize, active_rows: usize) -> RenderOptions {
    RenderOptions {
        width,
        active_rows,
        ..RenderOptions::default()
    }
}

fn png(width: u32, height: u32, pixels: &[u8], color: ExtendedColorType) -> Vec<u8> {
    let mut bytes = Vec::new();
    PngEncoder::new(&mut bytes)
        .write_image(pixels, width, height, color)
        .unwrap();
    bytes
}

fn header(width: u32, height: u32, depth: u8, color: u8) -> Vec<u8> {
    let mut bytes = fixture()[..33].to_vec();
    bytes[16..20].copy_from_slice(&width.to_be_bytes());
    bytes[20..24].copy_from_slice(&height.to_be_bytes());
    bytes[24] = depth;
    bytes[25] = color;
    // Deliberately no image data or corrected CRC: policy must reject these
    // headers before a decoder could report a CRC error or allocate pixels.
    bytes
}

#[test]
fn default_design_options() {
    assert_eq!(RenderOptions::default(), options(400, 30));
    assert_eq!(RenderOptions::default().threshold, 128);
    assert!(!RenderOptions::default().dither);
}

#[test]
fn fixed_png_matches_full_binary_golden_and_alpha_is_white() {
    let raster = render_png(&fixture(), options(2, 2)).unwrap();
    let golden: [u8; 64] = [
        0, 0, // 0
        0, 0, // 1
        0, 0, // 2
        0, 0, // 3
        0, 0, // 4
        0, 0, // 5
        0, 0, // 6
        0, 0, // 7
        0, 0, // 8
        0, 0, // 9
        0, 0, // 10
        0, 0, // 11
        0, 0, // 12
        0, 0, // 13
        0, 0, // 14
        1, 0, // 15: opaque black and white
        0, 1, // 16: transparent and half-transparent black
        0, 0, // 17
        0, 0, // 18
        0, 0, // 19
        0, 0, // 20
        0, 0, // 21
        0, 0, // 22
        0, 0, // 23
        0, 0, // 24
        0, 0, // 25
        0, 0, // 26
        0, 0, // 27
        0, 0, // 28
        0, 0, // 29
        0, 0, // 30
        0, 0, // 31
    ];
    assert_eq!(raster.width(), 2);
    assert_eq!(raster.pixels(), golden);
}

#[test]
fn grayscale_polarity_and_threshold_boundary() {
    let raster = render_gray(4, 1, &[0, 127, 128, 255], options(4, 1)).unwrap();
    let mut golden = [0; 128];
    golden[60..64].copy_from_slice(&[1, 1, 0, 0]);
    assert_eq!(raster.pixels(), golden);
    let white = render_gray(
        1,
        1,
        &[0],
        RenderOptions {
            threshold: 0,
            ..options(1, 1)
        },
    )
    .unwrap();
    assert!(white.pixels().iter().all(|&pixel| pixel == 0));
}

#[test]
fn aspect_ratio_is_preserved_with_horizontal_and_vertical_centering() {
    // A 2:1 source fits to 8x4, not the requested 8x8 box.
    let wide = render_gray(2, 1, &[0, 0], options(8, 8)).unwrap();
    let mut wide_golden = vec![0; 8 * 32];
    wide_golden[14 * 8..18 * 8].fill(1);
    assert_eq!(wide.pixels(), wide_golden);
    // A 1:2 source fits to 4x8, centered across eight feed columns.
    let tall = render_gray(1, 2, &[0, 0], options(8, 8)).unwrap();
    let mut tall_golden = vec![0; 8 * 32];
    for row in tall_golden
        .as_chunks_mut::<8>()
        .0
        .iter_mut()
        .skip(12)
        .take(8)
    {
        row.copy_from_slice(&[0, 0, 1, 1, 1, 1, 0, 0]);
    }
    assert_eq!(tall.pixels(), tall_golden);
}

#[test]
fn source_white_margins_are_not_trimmed() {
    let raster = render_gray(4, 1, &[255, 0, 0, 255], options(8, 8)).unwrap();
    let mut golden = vec![0; 8 * 32];
    golden[15 * 8..16 * 8].copy_from_slice(&[0, 0, 1, 1, 1, 1, 0, 0]);
    golden[16 * 8..17 * 8].copy_from_slice(&[0, 0, 1, 1, 1, 1, 0, 0]);
    assert_eq!(raster.pixels(), golden);
}

#[test]
fn active_rows_are_centered_on_full_head() {
    for active_rows in 1..=32 {
        let raster = render_gray(1, 1, &[0], options(64, active_rows)).unwrap();
        let left = (64 - active_rows) / 2;
        let top = (32 - active_rows) / 2;
        let mut golden = vec![0; 64 * 32];
        for y in top..top + active_rows {
            golden[y * 64 + left..y * 64 + left + active_rows].fill(1);
        }
        assert_eq!(raster.pixels(), golden, "active_rows={active_rows}");
    }
}

#[test]
fn extreme_aspect_ratio_retains_at_least_one_pixel() {
    for (width, height) in [(8192, 1), (1, 8192)] {
        let raster = render_gray(width, height, &vec![0; width * height], options(1, 1)).unwrap();
        let mut golden = [0; 32];
        golden[15] = 1;
        assert_eq!(raster.pixels(), golden);
    }
}

#[test]
fn downsampling_uses_pixel_centers() {
    let raster = render_gray(4, 1, &[255, 0, 255, 0], options(2, 1)).unwrap();
    assert!(raster.get(0, 15));
    assert!(raster.get(1, 15));
    assert_eq!(raster.pixels().iter().filter(|&&p| p == 1).count(), 2);
}

#[test]
fn bayer_pattern_is_deterministic_and_golden() {
    let options = RenderOptions {
        dither: true,
        ..options(4, 4)
    };
    let raster = render_gray(4, 4, &[128; 16], options).unwrap();
    let mut golden = [0; 128];
    golden[56..72].copy_from_slice(&[0, 1, 0, 1, 1, 0, 1, 0, 0, 1, 0, 1, 1, 0, 1, 0]);
    assert_eq!(raster.pixels(), golden);
    assert_eq!(
        raster.pixels(),
        render_gray(4, 4, &[128; 16], options).unwrap().pixels()
    );
    for (gray, black) in [(0, 1), (255, 0)] {
        let raster = render_gray(4, 4, &[gray; 16], options).unwrap();
        assert!(raster.pixels()[56..72].iter().all(|&pixel| pixel == black));
    }
}

#[test]
fn colored_alpha_is_composited_before_luminance() {
    let bytes = png(
        4,
        1,
        &[255, 0, 0, 0, 255, 0, 0, 255, 0, 0, 0, 127, 0, 0, 0, 128],
        ExtendedColorType::Rgba8,
    );
    let raster = render_png(&bytes, options(4, 1)).unwrap();
    assert_eq!(&raster.pixels()[60..64], &[0, 1, 0, 1]);
}

#[test]
fn preview_contains_full_protocol_raster_and_is_byte_repeatable() {
    let raster = Raster::new(3, (0..96).map(|index| u8::from(index % 5 == 0)).collect()).unwrap();
    let first = preview_png(&raster).unwrap();
    assert_eq!(first, preview_png(&raster).unwrap());
    let decoded = image::load_from_memory_with_format(&first, image::ImageFormat::Png)
        .unwrap()
        .to_luma8();
    assert_eq!(decoded.dimensions(), (3, 32));
    for (index, pixel) in decoded.as_raw().iter().enumerate() {
        assert_eq!(*pixel, if raster.pixels()[index] == 0 { 255 } else { 0 });
    }
    assert_eq!(
        render_png(&first, options(3, 32)).unwrap().pixels(),
        raster.pixels()
    );
}

#[test]
fn output_dimensions_and_exact_grayscale_length_are_validated() {
    for width in [0, MAX_COLUMNS + 1, usize::MAX] {
        assert!(matches!(
            render_gray(1, 1, &[0], options(width, 1)),
            Err(Error::InvalidOutputWidth { .. })
        ));
        assert!(matches!(
            render_png(&fixture(), options(width, 1)),
            Err(Error::InvalidOutputWidth { .. })
        ));
    }
    for rows in [0, HEAD_ROWS + 1, usize::MAX] {
        assert!(matches!(
            render_gray(1, 1, &[0], options(1, rows)),
            Err(Error::InvalidActiveRows { .. })
        ));
    }
    for pixels in [&[][..], &[0, 0][..]] {
        assert!(matches!(
            render_gray(1, 1, pixels, options(1, 1)),
            Err(Error::GrayscaleLength { .. })
        ));
    }
    let max_width = render_gray(1, 1, &[0], options(MAX_COLUMNS, 32)).unwrap();
    assert_eq!(max_width.pixels().len(), MAX_COLUMNS * 32);
}

#[test]
fn source_dimension_and_pixel_bounds_precede_input_length() {
    for (width, height) in [
        (0, 1),
        (1, 0),
        (8193, 1),
        (1, 8193),
        (usize::MAX, usize::MAX),
    ] {
        assert!(matches!(
            render_gray(width, height, &[], options(1, 1)),
            Err(Error::InvalidSourceDimensions { .. })
        ));
    }
    assert!(matches!(
        render_gray(4001, 4000, &[], options(1, 1)),
        Err(Error::TooManyPixels { .. })
    ));
    assert!(matches!(
        render_gray(4000, 4000, &[], options(1, 1)),
        Err(Error::GrayscaleLength {
            expected: MAX_SOURCE_PIXELS,
            ..
        })
    ));
}

#[test]
fn png_size_format_and_truncation_are_rejected() {
    assert!(matches!(
        render_png(&vec![0; MAX_PNG_BYTES + 1], options(1, 1)),
        Err(Error::FileTooLarge { .. })
    ));
    assert!(!matches!(
        render_png(&vec![0; MAX_PNG_BYTES], options(1, 1)),
        Err(Error::FileTooLarge { .. })
    ));
    for bytes in [
        &b"%PDF-1.7"[..],
        &b"GIF89a"[..],
        &b"\xff\xd8\xff"[..],
        &[][..],
    ] {
        assert!(matches!(
            render_png(bytes, options(1, 1)),
            Err(Error::InvalidPngHeader)
        ));
    }
    let bytes = fixture();
    for length in 0..bytes.len() - 12 {
        assert!(
            render_png(&bytes[..length], options(2, 2)).is_err(),
            "length={length}"
        );
    }
}

#[test]
fn oversized_png_headers_are_rejected_before_decoder_allocation() {
    for (width, height) in [(0, 1), (1, 0), (8193, 1), (1, 8193), (u32::MAX, u32::MAX)] {
        assert!(matches!(
            render_png(&header(width, height, 8, 6), options(1, 1)),
            Err(Error::InvalidSourceDimensions { .. })
        ));
    }
    assert!(matches!(
        render_png(&header(4001, 4000, 8, 6), options(1, 1)),
        Err(Error::TooManyPixels { .. })
    ));
    assert!(matches!(
        render_png(&header(4000, 4000, 16, 6), options(1, 1)),
        Err(Error::AllocationLimit { bytes: 128_000_000 })
    ));
    // 8-bit RGBA at the pixel boundary fits the allocation cap and reaches the
    // decoder (which rejects our intentionally stale header CRC).
    const { assert!(64_000_000 < MAX_DECODE_BYTES) };
    assert!(matches!(
        render_png(&header(4000, 4000, 8, 6), options(1, 1)),
        Err(Error::Image(_))
    ));
}

proptest! {
    #[test]
    fn arbitrary_small_gray_is_binary_and_bounded(
        (width, height, pixels) in (1_usize..65, 1_usize..65)
            .prop_flat_map(|(w, h)| (Just(w), Just(h), prop::collection::vec(any::<u8>(), w * h))),
        output_width in 1_usize..=MAX_COLUMNS,
        active_rows in 1_usize..=HEAD_ROWS,
        threshold in any::<u8>(),
        dither in any::<bool>(),
    ) {
        let options = RenderOptions { width: output_width, active_rows, threshold, dither };
        let raster = render_gray(width, height, &pixels, options).unwrap();
        prop_assert_eq!(raster.width(), output_width);
        prop_assert_eq!(raster.pixels().len(), output_width * HEAD_ROWS);
        prop_assert!(raster.pixels().iter().all(|&p| p <= 1));
        let black_rows: Vec<_> = raster.pixels().chunks_exact(output_width)
            .enumerate().filter(|(_, row)| row.contains(&1)).map(|(y, _)| y).collect();
        if let (Some(first), Some(last)) = (black_rows.first(), black_rows.last()) {
            prop_assert!(last - first < active_rows);
        }
    }

    #[test]
    fn arbitrary_invalid_output_width_is_rejected(width in (MAX_COLUMNS + 1)..=usize::MAX) {
        prop_assert!(matches!(render_gray(1, 1, &[0], options(width, 1)), Err(Error::InvalidOutputWidth { .. })), "invalid width was accepted");
    }

    #[test]
    fn arbitrary_invalid_active_rows_are_rejected(rows in (HEAD_ROWS + 1)..=usize::MAX) {
        prop_assert!(matches!(render_gray(1, 1, &[0], options(1, rows)), Err(Error::InvalidActiveRows { .. })), "invalid row count was accepted");
    }

    #[test]
    fn arbitrary_invalid_source_dimensions_are_rejected(width in 8193_usize..=usize::MAX, height in any::<usize>()) {
        prop_assert!(matches!(render_gray(width, height, &[], options(1, 1)), Err(Error::InvalidSourceDimensions { .. })), "invalid source dimensions were accepted");
    }

    #[test]
    fn arbitrary_incorrect_gray_lengths_are_rejected(width in 1_usize..65, height in 1_usize..65, extra in 1_usize..16) {
        let pixels = vec![0; width * height + extra];
        prop_assert!(matches!(render_gray(width, height, &pixels, options(1, 1)), Err(Error::GrayscaleLength { .. })), "incorrect grayscale length was accepted");
    }

    #[test]
    fn arbitrary_bytes_never_panic(bytes in prop::collection::vec(any::<u8>(), 0..2048)) {
        let _ = render_png(&bytes, options(4, 4));
    }
}
