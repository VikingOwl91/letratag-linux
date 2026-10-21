//! Payload preservation regression tests.
//!
//! The retained fixtures are the exact wire captures of the two verified
//! physical diagnostics:
//!
//! * `hello-linux-legacy.ble`: the original 2026-10-20 `HELLO LINUX` payload
//!   (legacy profile, validated 244-byte cap), reproduced byte-for-byte by
//!   the 2026-10-21 historical reproduction print. Wire SHA-256
//!   `357a6b8d…eeb98b`, body SHA-256 `58272474…069c53`, body 1624 bytes,
//!   writes `[9,242,242,242,242,242,242,181]`, raster SHA-256 `76365e36…1927`
//!   (152 x 16 occupied ink pixels at x124..275/y8..23).
//! * `raw-raster-diagnostic.ble`: the 2026-10-21 RAW raster diagnostic
//!   payload (in-memory 400x32 raster with the 160-feed-column reference
//!   line, legacy profile, validated 244-byte cap). Wire SHA-256
//!   `2720fc13…fe057`, body SHA-256 `0e2f7648…b340b`.
//!
//! These tests prove the encoder produces byte-identical payloads for the
//! same raster, profile and write limit, independent of any BLE transport.

use letratag_protocol::{Job, Profile, Raster};
use letratag_render::{RenderOptions, render_png};

/// Decode a length-prefixed write capture into the legacy body and raster.
fn decode_capture(data: &[u8]) -> (Vec<u8>, Raster) {
    let mut cursor = 0usize;
    let mut writes: Vec<&[u8]> = Vec::new();
    while cursor < data.len() {
        let size = u16::from_le_bytes([data[cursor], data[cursor + 1]]) as usize;
        cursor += 2;
        assert!((1..=512).contains(&size), "invalid write length");
        writes.push(&data[cursor..cursor + size]);
        cursor += size;
    }
    assert_eq!(writes[0].len(), 9, "header must be 9 bytes");
    assert_eq!(&writes[0][..4], &[0xff, 0xf0, 0x12, 0x34]);
    let mut body = Vec::new();
    for (i, write) in writes[1..].iter().enumerate() {
        assert_eq!(
            write[0] as usize,
            i + usize::from(i >= 27),
            "chunk index sequence"
        );
        let end = if i == writes.len() - 2 {
            assert_eq!(&write[write.len() - 2..], &[0x12, 0x34]);
            write.len() - 2
        } else {
            write.len()
        };
        body.extend_from_slice(&write[1..end]);
    }
    assert_eq!(
        body.len(),
        u32::from_le_bytes(writes[0][4..8].try_into().unwrap()) as usize
    );
    // Legacy profile: ESC s preamble, ESC D, raster, ESC E, ESC A, ESC Q.
    assert_eq!(&body[..6], &[0x1b, 0x73, 0x9a, 0x02, 0x00, 0x00]);
    assert_eq!(&body[6..8], &[0x1b, 0x44]);
    assert_eq!(body[8], 0x01, "legacy mode byte");
    assert_eq!(body[9], 0x02, "alignment");
    let width = u32::from_le_bytes(body[10..14].try_into().unwrap()) as usize;
    let height = u32::from_le_bytes(body[14..18].try_into().unwrap()) as usize;
    assert_eq!(height, 32);
    let raster = Raster::from_packed_columns(width, &body[18..18 + width * 4]).unwrap();
    assert_eq!(
        &body[18 + width * 4..],
        &[0x1b, 0x45, 0x1b, 0x41, 0x1b, 0x51]
    );
    (body, raster)
}

/// Re-encode a raster with the legacy profile and the validated 244-byte cap,
/// then re-frame it as a length-prefixed capture.
fn encode_capture(raster: &Raster) -> Vec<u8> {
    let job = Job::new(raster, Profile::Legacy).unwrap();
    let writes = job.writes(244).unwrap();
    let mut capture = Vec::new();
    for write in &writes {
        capture.extend_from_slice(&(write.len() as u16).to_le_bytes());
        capture.extend_from_slice(write);
    }
    capture
}

#[test]
fn hello_linux_payload_is_preserved_from_the_retained_capture() {
    let fixture = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/hello-linux-legacy.ble"
    ))
    .unwrap();
    let (body, raster) = decode_capture(&fixture);
    assert_eq!(body.len(), 1624);
    // 152 x 16 occupied ink pixels at x124..275/y8..23 (center span 151x15).
    let xs: Vec<usize> = (0..raster.width())
        .filter(|&x| (0..32).any(|y| raster.get(x, y)))
        .collect();
    let ys: Vec<usize> = (0..32)
        .filter(|&y| (0..raster.width()).any(|x| raster.get(x, y)))
        .collect();
    assert_eq!((*xs.first().unwrap(), *xs.last().unwrap()), (124, 275));
    assert_eq!((*ys.first().unwrap(), *ys.last().unwrap()), (8, 23));
    assert_eq!(xs.last().unwrap() - xs.first().unwrap() + 1, 152);
    assert_eq!(ys.last().unwrap() - ys.first().unwrap() + 1, 16);
    // The encoder reproduces the retained capture byte-for-byte.
    assert_eq!(encode_capture(&raster), fixture);
}

#[test]
fn hello_linux_payload_is_preserved_from_the_original_png_render_chain() {
    let png = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/hello-linux.png"
    ))
    .unwrap();
    let raster = render_png(
        &png,
        RenderOptions {
            width: 400,
            ..Default::default()
        },
    )
    .unwrap();
    let fixture = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/hello-linux-legacy.ble"
    ))
    .unwrap();
    assert_eq!(
        encode_capture(&raster),
        fixture,
        "the original PNG render chain must produce the retained payload"
    );
}

#[test]
fn raw_diagnostic_payload_is_preserved_from_the_retained_capture() {
    let fixture = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/raw-raster-diagnostic.ble"
    ))
    .unwrap();
    let (body, raster) = decode_capture(&fixture);
    assert_eq!(body.len(), 1624);
    assert_eq!(raster.width(), 400);
    // The reference line occupies exactly 160 feed columns at row 16.
    let line: Vec<usize> = (0..raster.width()).filter(|&x| raster.get(x, 16)).collect();
    assert_eq!(line.len(), 160);
    assert_eq!((*line.first().unwrap(), *line.last().unwrap()), (20, 179));
    assert!(!raster.get(19, 16));
    assert!(!raster.get(180, 16));
    // Text ink present in rows 3..=9.
    assert!((3..=9).any(|y| (0..raster.width()).any(|x| raster.get(x, y))));
    // The encoder reproduces the retained capture byte-for-byte.
    assert_eq!(encode_capture(&raster), fixture);
}

#[test]
fn both_reference_payloads_share_command_order_and_chunk_boundaries() {
    let hello = decode_capture(
        &std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/hello-linux-legacy.ble"
        ))
        .unwrap(),
    );
    let raw = decode_capture(
        &std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/raw-raster-diagnostic.ble"
        ))
        .unwrap(),
    );
    for (a, b) in [&hello.0, &raw.0].iter().zip([&hello.0, &raw.0]) {
        let _ = (a, b);
    }
    // Same frame geometry and command sequence; only raster content differs.
    assert_eq!(hello.0.len(), raw.0.len());
    assert_eq!(&hello.0[..18], &raw.0[..18], "identical ESC D header");
    assert_eq!(
        &hello.0[hello.0.len() - 6..],
        &raw.0[raw.0.len() - 6..],
        "identical command suffix"
    );
    assert_ne!(
        &hello.0[18..18 + 1600],
        &raw.0[18..18 + 1600],
        "raster contents differ as intended"
    );
    // Identical chunk boundaries at the validated 244-byte cap.
    let hello_writes = Job::new(&hello.1, Profile::Legacy)
        .unwrap()
        .writes(244)
        .unwrap();
    let raw_writes = Job::new(&raw.1, Profile::Legacy)
        .unwrap()
        .writes(244)
        .unwrap();
    let hello_sizes: Vec<usize> = hello_writes.iter().map(|w| w.len()).collect();
    let raw_sizes: Vec<usize> = raw_writes.iter().map(|w| w.len()).collect();
    assert_eq!(hello_sizes, vec![9, 242, 242, 242, 242, 242, 242, 181]);
    assert_eq!(raw_sizes, hello_sizes);
}
