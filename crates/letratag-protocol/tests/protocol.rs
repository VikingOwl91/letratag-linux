// Wire references (MIT), inspected at these revisions, not hardware validation:
// thermal2026 / letratag, packages/core/src/protocol.ts:
// a8fb41e19ce3a8cb1e51d6aaeb1acc8060d5842b (packing, modern commands, framing).
// ysfchn / dymo-bluetooth, dymo_bluetooth/printer.py:
// 1c1b30a6612de6bf880d5f077a23630b83810750 (legacy commands, framing, skip 27).
// The literals below were transcribed independently, including header checksums.

use letratag_protocol::{
    Error, HEAD_ROWS, Job, MAX_COLUMNS, Profile, Raster, Status, parse_header,
    set_cassette_type_write,
};
use proptest::prelude::*;

fn blank(width: usize) -> Raster {
    Raster::new(width, vec![0; width * HEAD_ROWS]).unwrap()
}

#[test]
fn set_cassette_type_write_is_a_single_header_plus_body_write() {
    // S2 docs/protocol/letratag-bt.md: HEADER[9] + ESC s + ESC M 03 00 00 00
    // + ESC Q, one non-print write, 23 bytes total. Body length 14, checksum
    // (255+240+18+52+14) % 256 = 0x43.
    let write = set_cassette_type_write(3).unwrap();
    assert_eq!(write.len(), 23);
    assert_eq!(
        write,
        vec![
            0xff, 0xf0, 0x12, 0x34, 0x0e, 0, 0, 0, 0x43, // header, body length 14
            0x1b, 0x73, 0x9a, 0x02, 0x00, 0x00, // ESC s (start)
            0x1b, 0x4d, 0x03, 0x00, 0x00, 0x00, // ESC M (12 mm)
            0x1b, 0x51, // ESC Q (end)
        ]
    );
    assert_eq!(parse_header(&write[..9]), Ok(14));
    for id in [0, 6, 255] {
        assert!(matches!(
            set_cassette_type_write(id),
            Err(Error::InvalidCassetteId { .. })
        ));
    }
}

#[test]
fn complete_one_column_vectors() {
    let mut pixels = vec![0; HEAD_ROWS];
    pixels[0] = 1;
    let raster = Raster::new(1, pixels).unwrap();
    // Modern length 32, checksum (255+240+18+52+32) % 256 = 0x55.
    let modern = vec![
        vec![0xff, 0xf0, 0x12, 0x34, 0x20, 0, 0, 0, 0x55],
        vec![
            0x00, // chunk index
            0x1b, 0x73, 0x9a, 0x02, 0x00, 0x00, 0x1b, 0x23, 0x01, 0x1b, 0x44, 0x81, 0x02, 1, 0, 0,
            0, 32, 0, 0, 0, 0, 0, 0, 0x80, 0x1b, 0x70, 0x30, 0x1b, 0x41, 0x1b, 0x51, 0x12, 0x34,
        ],
    ];
    // Legacy length 28, checksum (255+240+18+52+28) % 256 = 0x51.
    let legacy = vec![
        vec![0xff, 0xf0, 0x12, 0x34, 0x1c, 0, 0, 0, 0x51],
        vec![
            0x00, 0x1b, 0x73, 0x9a, 0x02, 0x00, 0x00, 0x1b, 0x44, 0x01, 0x02, 1, 0, 0, 0, 32, 0, 0,
            0, 0, 0, 0, 0x80, 0x1b, 0x45, 0x1b, 0x41, 0x1b, 0x51, 0x12, 0x34,
        ],
    ];
    assert_eq!(
        Job::new(&raster, Profile::Modern)
            .unwrap()
            .writes(512)
            .unwrap(),
        modern
    );
    assert_eq!(
        Job::new(&raster, Profile::Legacy)
            .unwrap()
            .writes(512)
            .unwrap(),
        legacy
    );
    assert_eq!(parse_header(&modern[0]), Ok(32));
    assert_eq!(parse_header(&legacy[0]), Ok(28));
    assert_eq!(Profile::default(), Profile::Modern);
}

#[test]
fn singleton_bits_and_column_order() {
    for (y, expected) in [
        (0, [0, 0, 0, 0x80]),
        (7, [0, 0, 0, 0x01]),
        (8, [0, 0, 0x80, 0]),
        (24, [0x80, 0, 0, 0]),
        (31, [0x01, 0, 0, 0]),
    ] {
        for x in 0..3 {
            let mut pixels = vec![0; 3 * HEAD_ROWS];
            pixels[y * 3 + x] = 1;
            let raster = Raster::new(3, pixels).unwrap();
            let mut packed = vec![0; 12];
            packed[x * 4..x * 4 + 4].copy_from_slice(&expected);
            assert_eq!(raster.pack_columns(), packed);
            assert_eq!(Raster::from_packed_columns(3, &packed).unwrap(), raster);
            assert!(raster.get(x, y));
        }
    }
}

#[test]
fn raster_validation_is_bounded_and_binary() {
    for width in [0, MAX_COLUMNS + 1, usize::MAX / HEAD_ROWS + 1, usize::MAX] {
        assert!(matches!(
            Raster::new(width, vec![]),
            Err(Error::InvalidWidth { .. })
        ));
        assert!(matches!(
            Raster::from_packed_columns(width, &[]),
            Err(Error::InvalidWidth { .. })
        ));
    }
    for length in [0, 31, 33] {
        assert!(matches!(
            Raster::new(1, vec![0; length]),
            Err(Error::PixelLength { .. })
        ));
    }
    for value in 2..=255 {
        let mut pixels = vec![0; HEAD_ROWS];
        pixels[17] = value;
        assert_eq!(
            Raster::new(1, pixels),
            Err(Error::PixelValue { index: 17, value })
        );
    }
    assert!(Raster::from_packed_columns(1, &[0; 3]).is_err());
    assert!(Raster::from_packed_columns(1, &[0; 5]).is_err());
    let raster = blank(MAX_COLUMNS);
    assert_eq!(raster.width(), MAX_COLUMNS);
    assert_eq!(raster.pixels().len(), MAX_COLUMNS * HEAD_ROWS);
    for (x, y) in [(MAX_COLUMNS, 0), (0, HEAD_ROWS), (usize::MAX, usize::MAX)] {
        assert!(!raster.get(x, y));
    }
    for profile in [Profile::Modern, Profile::Legacy] {
        let job = Job::new(&raster, profile).unwrap();
        assert!(job.writes(512).is_ok());
        assert!(job.writes(20).is_err());
    }
}

fn header_with_length(length: u32) -> Vec<u8> {
    let mut header = vec![0xff, 0xf0, 0x12, 0x34];
    header.extend_from_slice(&length.to_le_bytes());
    let sum: u32 = header.iter().map(|&byte| u32::from(byte)).sum();
    header.push((sum % 256) as u8);
    header
}

#[test]
fn malformed_headers_and_bounded_lengths() {
    for length in [0, 1, 8, 10, 100] {
        assert!(matches!(
            parse_header(&vec![0; length]),
            Err(Error::HeaderLength { .. })
        ));
    }
    let header = header_with_length(32);
    for index in 0..9 {
        let mut bad = header.clone();
        bad[index] ^= 1;
        assert!(parse_header(&bad).is_err());
    }
    let max = (MAX_COLUMNS * 4 + 28) as u32;
    for length in [0, 1, max] {
        assert_eq!(
            parse_header(&header_with_length(length)),
            Ok(length as usize)
        );
    }
    for length in [max + 1, u32::MAX] {
        assert!(matches!(
            parse_header(&header_with_length(length)),
            Err(Error::BodyTooLarge { .. })
        ));
    }
}

#[test]
fn status_preserves_every_unknown_byte() {
    assert_eq!(Status::parse(&[0x1b, 0x52, 0]), Ok(Status::Success));
    for raw in 1..=255 {
        assert_eq!(
            Status::parse(&[0x1b, 0x52, raw]),
            Ok(Status::Unconfirmed(raw))
        );
    }
    for length in [0, 1, 2, 4, 9, 100] {
        assert!(matches!(
            Status::parse(&vec![0; length]),
            Err(Error::StatusLength { .. })
        ));
    }
    assert_eq!(Status::parse(&[0, 0x52, 0]), Err(Error::StatusPrefix));
    assert_eq!(Status::parse(&[0x1b, 0, 0]), Err(Error::StatusPrefix));
}

fn assert_reconstruction(job: &Job, max_write: usize) {
    let writes = job.writes(max_write).unwrap();
    assert_eq!(parse_header(&writes[0]), Ok(job.body().len()));
    let chunks = &writes[1..];
    let ceiling = 500.min(max_write - 3);
    assert_eq!(chunks.len(), job.body().len().div_ceil(ceiling));
    let mut reconstructed = Vec::new();
    for (i, chunk) in chunks.iter().enumerate() {
        assert!(chunk.len() <= max_write);
        assert_eq!(chunk[0] as usize, if i >= 27 { i + 1 } else { i });
        assert_ne!(chunk[0], 27);
        let end = if i + 1 == chunks.len() {
            assert_eq!(&chunk[chunk.len() - 2..], &[0x12, 0x34]);
            chunk.len() - 2
        } else {
            assert_eq!(chunk.len(), ceiling + 1);
            chunk.len()
        };
        reconstructed.extend_from_slice(&chunk[1..end]);
    }
    assert_eq!(reconstructed, job.body());
}

#[test]
fn write_bounds_trailer_and_skip_boundaries() {
    let job = Job::new(&blank(1), Profile::Modern).unwrap();
    for max_write in [0, 1, 8, 513, usize::MAX] {
        assert!(matches!(
            job.writes(max_write),
            Err(Error::InvalidWriteLength { .. })
        ));
    }
    // Modern widths 9, 10, 11 give body lengths 64, 68, 72. At max_write 20,
    // width 10 fills its final 17-byte body chunk exactly, including trailer.
    for (width, count, final_length) in [(9, 4, 16), (10, 4, 20), (11, 5, 7)] {
        let job = Job::new(&blank(width), Profile::Modern).unwrap();
        let writes = job.writes(20).unwrap();
        assert_eq!(writes.len(), count + 1);
        assert_eq!(writes.last().unwrap().len(), final_length);
        assert_reconstruction(&job, 20);
    }
    for width in [107, 108, 112] {
        assert_reconstruction(&Job::new(&blank(width), Profile::Modern).unwrap(), 20);
    }
    for (width, max_write) in [(375, 9), (1073, 20)] {
        let job = Job::new(&blank(width), Profile::Modern).unwrap();
        let writes = job.writes(max_write).unwrap();
        assert_eq!(writes.len(), 256); // header plus 255 body chunks
        assert_eq!(writes[27][0], 26);
        assert_eq!(writes[28][0], 28);
        assert_eq!(writes[255][0], 255);
        assert_reconstruction(&job, max_write);
    }
    for (width, max_write) in [(376, 9), (1077, 20)] {
        let job = Job::new(&blank(width), Profile::Modern).unwrap();
        assert_eq!(
            job.writes(max_write),
            Err(Error::TooManyChunks { count: 256 })
        );
    }
    for profile in [Profile::Modern, Profile::Legacy] {
        for max_write in [9, 20, 23, 244, 500, 503, 512] {
            assert_reconstruction(&Job::new(&blank(100), profile).unwrap(), max_write);
        }
        assert_reconstruction(&Job::new(&blank(MAX_COLUMNS), profile).unwrap(), 512);
    }
}

proptest! {
    #[test]
    fn arbitrary_headers_never_panic(bytes in prop::collection::vec(any::<u8>(), 0..2048)) {
        if let Ok(length) = parse_header(&bytes) {
            prop_assert_eq!(bytes.len(), 9);
            prop_assert!(length <= MAX_COLUMNS * 4 + 28);
        }
    }

    #[test]
    fn arbitrary_header_length_fields(length in any::<u32>()) {
        let result = parse_header(&header_with_length(length));
        prop_assert_eq!(result.is_ok(), u64::from(length) <= (MAX_COLUMNS * 4 + 28) as u64);
    }

    #[test]
    fn invalid_dimensions_reject_before_arithmetic(width in any::<usize>()) {
        prop_assume!(width == 0 || width > MAX_COLUMNS);
        prop_assert!(Raster::new(width, vec![]).is_err());
        prop_assert!(Raster::from_packed_columns(width, &[]).is_err());
    }

    #[test]
    fn packed_roundtrip(packed in prop::collection::vec(any::<[u8; 4]>(), 1..256)) {
        let width = packed.len();
        let bytes: Vec<u8> = packed.into_iter().flatten().collect();
        let raster = Raster::from_packed_columns(width, &bytes).unwrap();
        prop_assert!(raster.pixels().iter().all(|&value| value <= 1));
        prop_assert_eq!(raster.pack_columns(), bytes);
    }

    #[test]
    fn chunking_reconstructs_body(width in 1usize..=MAX_COLUMNS, max_write in 9usize..=512, legacy in any::<bool>()) {
        let profile = if legacy { Profile::Legacy } else { Profile::Modern };
        let job = Job::new(&blank(width), profile).unwrap();
        let count = job.body().len().div_ceil(500.min(max_write - 3));
        if count > 255 {
            prop_assert_eq!(job.writes(max_write), Err(Error::TooManyChunks { count }));
        } else {
            assert_reconstruction(&job, max_write);
        }
    }
}
