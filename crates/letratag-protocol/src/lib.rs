//! Offline LetraTag LT-200B wire encoding. Not verified against real hardware.
#![doc = include_str!("../README.md")]

use std::{error, fmt};

pub const HEAD_ROWS: usize = 32;
/// Software resource limit, not a claim about the printer's capabilities.
pub const MAX_COLUMNS: usize = 8000;

const BYTES_PER_COLUMN: usize = HEAD_ROWS / 8;
const MAX_BODY_LENGTH: usize = MAX_COLUMNS * BYTES_PER_COLUMN + 28;
const MAGIC: [u8; 2] = [0x12, 0x34];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    InvalidWidth { width: usize },
    PixelLength { expected: usize, actual: usize },
    PixelValue { index: usize, value: u8 },
    PackedLength { expected: usize, actual: usize },
    HeaderLength { actual: usize },
    HeaderMagic,
    HeaderChecksum,
    BodyTooLarge { length: usize },
    InvalidWriteLength { max_write: usize },
    TooManyChunks { count: usize },
    StatusLength { actual: usize },
    StatusPrefix,
    InvalidCassetteId { id: u8 },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidWidth { width } => {
                write!(f, "raster width {width} is outside 1..={MAX_COLUMNS}")
            }
            Self::PixelLength { expected, actual } => {
                write!(f, "expected {expected} pixels, got {actual}")
            }
            Self::PixelValue { index, value } => {
                write!(f, "pixel {index} has non-binary value {value}")
            }
            Self::PackedLength { expected, actual } => {
                write!(f, "expected {expected} packed bytes, got {actual}")
            }
            Self::HeaderLength { actual } => {
                write!(f, "expected a 9-byte header, got {actual} bytes")
            }
            Self::HeaderMagic => f.write_str("invalid header preamble or magic"),
            Self::HeaderChecksum => f.write_str("invalid header checksum"),
            Self::BodyTooLarge { length } => {
                write!(f, "body length {length} exceeds {MAX_BODY_LENGTH}")
            }
            Self::InvalidWriteLength { max_write } => {
                write!(f, "usable write length {max_write} is outside 9..=512")
            }
            Self::TooManyChunks { count } => {
                write!(f, "job requires {count} chunks; maximum is 255")
            }
            Self::StatusLength { actual } => {
                write!(f, "expected a 3-byte status, got {actual} bytes")
            }
            Self::StatusPrefix => f.write_str("invalid status prefix"),
            Self::InvalidCassetteId { id } => {
                write!(f, "cassette id {id} is outside 1..=5")
            }
        }
    }
}

impl error::Error for Error {}

/// A full 32-row frame, in row-major order, containing only 0 (white) or 1 (black).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Raster {
    width: usize,
    pixels: Vec<u8>,
}

impl Raster {
    pub fn new(width: usize, pixels: Vec<u8>) -> Result<Self, Error> {
        validate_width(width)?;
        let expected = width * HEAD_ROWS;
        if pixels.len() != expected {
            return Err(Error::PixelLength {
                expected,
                actual: pixels.len(),
            });
        }
        if let Some((index, &value)) = pixels.iter().enumerate().find(|(_, value)| **value > 1) {
            return Err(Error::PixelValue { index, value });
        }
        Ok(Self { width, pixels })
    }

    pub fn width(&self) -> usize {
        self.width
    }

    pub fn pixels(&self) -> &[u8] {
        &self.pixels
    }

    /// Out-of-frame coordinates are white, including overflowing coordinates.
    pub fn get(&self, x: usize, y: usize) -> bool {
        x < self.width && y < HEAD_ROWS && self.pixels[y * self.width + x] == 1
    }

    pub fn pack_columns(&self) -> Vec<u8> {
        let mut packed = vec![0; self.width * BYTES_PER_COLUMN];
        for x in 0..self.width {
            for y in 0..HEAD_ROWS {
                if self.get(x, y) {
                    packed[x * 4 + 3 - y / 8] |= 1 << (7 - y % 8);
                }
            }
        }
        packed
    }

    pub fn from_packed_columns(width: usize, packed: &[u8]) -> Result<Self, Error> {
        validate_width(width)?;
        let expected = width * BYTES_PER_COLUMN;
        if packed.len() != expected {
            return Err(Error::PackedLength {
                expected,
                actual: packed.len(),
            });
        }
        let mut pixels = vec![0; width * HEAD_ROWS];
        for x in 0..width {
            for y in 0..HEAD_ROWS {
                pixels[y * width + x] =
                    u8::from(packed[x * 4 + 3 - y / 8] & (1 << (7 - y % 8)) != 0);
            }
        }
        Self::new(width, pixels)
    }
}

fn validate_width(width: usize) -> Result<(), Error> {
    if !(1..=MAX_COLUMNS).contains(&width) {
        return Err(Error::InvalidWidth { width });
    }
    Ok(())
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum Profile {
    /// thermal2026: explicit single copy, 0x81 raster mode, cut-at-end command.
    #[default]
    Modern,
    /// ysfchn: 0x01 raster mode followed by form feed.
    Legacy,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Job {
    body: Vec<u8>,
}

impl Job {
    pub fn new(raster: &Raster, profile: Profile) -> Result<Self, Error> {
        let mut body = Vec::with_capacity(raster.width * BYTES_PER_COLUMN + 28);
        body.extend_from_slice(&[0x1b, 0x73, 0x9a, 0x02, 0x00, 0x00]);
        if profile == Profile::Modern {
            body.extend_from_slice(&[0x1b, 0x23, 0x01]);
        }
        let mode = if profile == Profile::Modern {
            0x81
        } else {
            0x01
        };
        body.extend_from_slice(&[0x1b, 0x44, mode, 0x02]);
        body.extend_from_slice(&(raster.width as u32).to_le_bytes());
        body.extend_from_slice(&(HEAD_ROWS as u32).to_le_bytes());
        body.extend_from_slice(&raster.pack_columns());
        match profile {
            Profile::Modern => body.extend_from_slice(&[0x1b, 0x70, 0x30]),
            Profile::Legacy => body.extend_from_slice(&[0x1b, 0x45]),
        }
        body.extend_from_slice(&[0x1b, 0x41, 0x1b, 0x51]);
        Ok(Self { body })
    }

    pub fn body(&self) -> &[u8] {
        &self.body
    }

    /// Ordered characteristic values: header first, then indexed body chunks.
    ///
    /// `max_write` is the usable characteristic value length, NOT the raw ATT MTU.
    /// Every chunk reserves one index byte and two final-magic bytes, even when
    /// non-final. A job may fit the raster limit but exceed the chunk index space.
    pub fn writes(&self, max_write: usize) -> Result<Vec<Vec<u8>>, Error> {
        if !(9..=512).contains(&max_write) {
            return Err(Error::InvalidWriteLength { max_write });
        }
        let ceiling = 500.min(max_write - 3);
        let count = self.body.len().div_ceil(ceiling);
        if count > 255 {
            return Err(Error::TooManyChunks { count });
        }
        let mut header = vec![0xff, 0xf0, MAGIC[0], MAGIC[1]];
        header.extend_from_slice(&(self.body.len() as u32).to_le_bytes());
        header.push(checksum(&header));
        let mut writes = Vec::with_capacity(count + 1);
        writes.push(header);
        for (i, bytes) in self.body.chunks(ceiling).enumerate() {
            let mut chunk = Vec::with_capacity(bytes.len() + 3);
            chunk.push(if i >= 27 { (i + 1) as u8 } else { i as u8 });
            chunk.extend_from_slice(bytes);
            if i + 1 == count {
                chunk.extend_from_slice(&MAGIC);
            }
            writes.push(chunk);
        }
        Ok(writes)
    }
}

fn checksum(bytes: &[u8]) -> u8 {
    bytes.iter().fold(0, |sum, byte| sum.wrapping_add(*byte))
}

/// Cassette id 3: 12 mm tape (the only width the LT-200B chassis accepts).
/// The id enum is shared by the `ESC M` directive and the advertising-data
/// cassette field (S2 docs/protocol/letratag-bt.md): 1=6mm, 2=9mm, 3=12mm,
/// 4=19mm, 5=24mm.
pub const CASSETTE_ID_12MM: u8 = 3;

/// Stand-alone set-cassette-type write for the short-command characteristic:
/// `HEADER + ESC s + ESC M <id> 00 00 00 + ESC Q` as one non-print write
/// (S2 docs/protocol/letratag-bt.md, "Stand-alone set-cassette-type payload").
/// The three zero pad bytes are part of the wire format. S1 documents the
/// same command with a 3-byte `ESC M` and no padding; that variant was
/// never bench-tested (S1's CLI does not use it), so the S2 form is used.
pub fn set_cassette_type_write(cassette_id: u8) -> Result<Vec<u8>, Error> {
    if !(1..=5).contains(&cassette_id) {
        return Err(Error::InvalidCassetteId { id: cassette_id });
    }
    let mut body = Vec::with_capacity(14);
    body.extend_from_slice(&[0x1b, 0x73, 0x9a, 0x02, 0x00, 0x00]);
    body.extend_from_slice(&[0x1b, 0x4d, cassette_id, 0x00, 0x00, 0x00]);
    body.extend_from_slice(&[0x1b, 0x51]);
    let mut write = vec![0xff, 0xf0, MAGIC[0], MAGIC[1]];
    write.extend_from_slice(&(body.len() as u32).to_le_bytes());
    write.push(checksum(&write));
    write.extend_from_slice(&body);
    Ok(write)
}

/// Validate header framing and checksum, returning a software-bounded body length.
/// This does not validate the body or assert that a length describes a print job.
pub fn parse_header(bytes: &[u8]) -> Result<usize, Error> {
    if bytes.len() != 9 {
        return Err(Error::HeaderLength {
            actual: bytes.len(),
        });
    }
    if bytes[..4] != [0xff, 0xf0, MAGIC[0], MAGIC[1]] {
        return Err(Error::HeaderMagic);
    }
    if checksum(&bytes[..8]) != bytes[8] {
        return Err(Error::HeaderChecksum);
    }
    let length = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]) as usize;
    if length > MAX_BODY_LENGTH {
        return Err(Error::BodyTooLarge { length });
    }
    Ok(length)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Success,
    Unconfirmed(u8),
}

impl Status {
    /// Only zero is recognized as success. Other codes retain their raw value.
    pub fn parse(bytes: &[u8]) -> Result<Self, Error> {
        match bytes {
            [0x1b, 0x52, 0] => Ok(Self::Success),
            [0x1b, 0x52, raw] => Ok(Self::Unconfirmed(*raw)),
            [_, _, _] => Err(Error::StatusPrefix),
            _ => Err(Error::StatusLength {
                actual: bytes.len(),
            }),
        }
    }
}
