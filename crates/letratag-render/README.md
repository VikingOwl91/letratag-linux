# letratag-render

Bounded, deterministic image rendering for the DYMO LetraTag 200B: PNG or
8-bit grayscale input becomes a 32-row binary raster in the wire format of
[`letratag-protocol`](https://crates.io/crates/letratag-protocol), and rasters
can be encoded back to PNG previews. No Bluetooth, CUPS, or hardware
dependency.

[![crates.io](https://img.shields.io/crates/v/letratag-render)](https://crates.io/crates/letratag-render)
[![repository](https://img.shields.io/badge/repository-GitHub-blue)](https://github.com/VikingOwl91/letratag-linux)

Part of [letratag-linux](https://github.com/VikingOwl91/letratag-linux).

## Install

```toml
[dependencies]
letratag-render = "0.1.0"
```

Rust 1.98 or newer. No default features; there are no feature flags. PNG
support uses the `image` crate; `letratag-protocol` provides the raster type.

## Examples

### Render grayscale samples

`render_gray` takes row-major 8-bit grayscale (0 is black, 255 is white).
Images keep their aspect ratio (rounded down to whole pixels, one-pixel
minimum) and are centered on the canvas; an odd spare pixel goes to the
right or bottom. Scaling samples nearest neighbors at pixel centers.

```rust
use letratag_render::{render_gray, RenderOptions};

let pixels = vec![0u8; 4 * 2]; // 4x2 black source
let raster = render_gray(4, 2, &pixels, RenderOptions::default()).unwrap();
assert_eq!(raster.width(), 400); // the default output width
// The 4x2 source scales to 60x30 and is centered on the 400x32 canvas.
assert!(raster.get(200, 16));
assert!(!raster.get(0, 0));
```

### Render a PNG and build a preview

`render_png` decodes only PNG (header dimensions are checked before decoder
construction, and decoder buffers are bounded). `preview_png` encodes the
actual full-height raster, without rescaling or trimming, as a grayscale PNG.

```rust
use letratag_render::{preview_png, render_gray, render_png, RenderOptions};

let raster = render_gray(2, 2, &[0, 255, 255, 0], RenderOptions::default()).unwrap();
let png = preview_png(&raster).unwrap();
assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");

// The preview is a full-height grayscale PNG that decodes again.
let roundtrip = render_png(&png, RenderOptions::default()).unwrap();
assert_eq!(roundtrip.width(), raster.width());
```

### Options and validation errors

`RenderOptions` selects the output width (feed columns), the number of active
rows across the tape, the luminance threshold, and optional deterministic
4x4 Bayer dithering. Invalid options and mismatched input are rejected with
structured errors before any rendering.

```rust
use letratag_render::{render_gray, Error, RenderOptions};

let mut options = RenderOptions::default();
options.width = 0;
assert!(matches!(
    render_gray(1, 1, &[0], options),
    Err(Error::InvalidOutputWidth { width: 0 })
));
assert!(matches!(
    render_gray(1, 1, &[0, 0], RenderOptions::default()),
    Err(Error::GrayscaleLength { .. })
));
```

## API overview

| Item | Purpose |
|---|---|
| `render_png` | Decode a bounded PNG into a `Raster` |
| `render_gray` | Render row-major 8-bit grayscale into a `Raster` |
| `preview_png` | Encode a `Raster` as a full-height grayscale PNG |
| `RenderOptions` | Output width, active rows, threshold, dither (`Default`: 400 columns, 30 rows, 128, no dither) |
| `Error` | Bounded, structured rendering errors |
| `MAX_PNG_BYTES`, `MAX_SOURCE_DIMENSION`, `MAX_SOURCE_PIXELS`, `MAX_DECODE_BYTES` | Input and decoder resource limits |

## Limitations

* PNG only. No JPEG, text, font, or PDF rendering. For APNG, only the default
  image is rendered, not animation frames.
* Rendering is aspect-ratio preserving with thresholding or Bayer dithering.
  It never stretches, crops silently, or fits to page; content outside the
  active rows is a design choice, not a physical printhead calibration.
* The 30 active rows are a design convention (S1's canvas), not a measured
  printhead limit; the full 32-row wire frame is always transmitted.
* This crate renders images; it does not print. Feeding a raster to a printer
  is the job of a transport (see `letratag-transport`), and physical print
  completion is never proven by a raster or a preview.

## License

MIT. See the [main repository](https://github.com/VikingOwl91/letratag-linux)
for the full license text and the reference-notice inventory in
`docs/legal/`.
