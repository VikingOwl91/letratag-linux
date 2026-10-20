use letratag_protocol::{MAX_COLUMNS, Raster};

#[derive(Debug, thiserror::Error)]
#[error("unsupported, incomplete or out-of-printable-area page")]
pub struct Error;

/// Generic page storage. PWG rows run along the tape; columns span its width.
pub struct Page {
    width: usize,
    height: usize,
    next_y: usize,
    black_is_zero: bool,
    failed: bool,
    pixels: Vec<u8>,
}

impl Page {
    pub fn new(width: usize, height: usize, bits: u32, black_is_zero: bool) -> Result<Self, Error> {
        if !(30..=128).contains(&width) || !(1..=MAX_COLUMNS).contains(&height) || bits != 8 {
            return Err(Error);
        }
        Ok(Self {
            width,
            height,
            next_y: 0,
            black_is_zero,
            failed: false,
            pixels: vec![255; width * height],
        })
    }
    pub fn line(&mut self, y: usize, data: &[u8]) -> Result<(), Error> {
        if self.failed || y != self.next_y || y >= self.height || data.len() != self.width {
            self.failed = true;
            return Err(Error);
        }
        for (out, value) in self.pixels[y * self.width..(y + 1) * self.width]
            .iter_mut()
            .zip(data)
        {
            *out = if self.black_is_zero {
                *value
            } else {
                255 - *value
            };
        }
        self.next_y += 1;
        Ok(())
    }
    pub fn raster(&self) -> Result<Raster, Error> {
        if self.failed || self.next_y != self.height {
            return Err(Error);
        }
        let start = (self.width - 30) / 2;
        let mut mono = vec![0; self.height * 32];
        for feed in 0..self.height {
            for cross in 0..self.width {
                if self.pixels[feed * self.width + cross] >= 128 {
                    continue;
                }
                if cross < start || cross >= start + 30 {
                    return Err(Error);
                }
                // Portrait -> landscape rotation, without resampling or page fitting.
                let head = 30 - (cross - start);
                mono[head * self.height + feed] = 1;
            }
        }
        Raster::new(self.height, mono).map_err(|_| Error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn portrait_rotation_and_no_rescaling() {
        let mut page = Page::new(94, 40, 8, true).unwrap();
        for y in 0..40 {
            let mut row = vec![255; 94];
            if y == 2 {
                row[32] = 0;
            }
            page.line(y, &row).unwrap();
        }
        let raster = page.raster().unwrap();
        assert_eq!(raster.width(), 40);
        assert!(raster.get(2, 30));
        assert_eq!(raster.pixels().iter().filter(|&&p| p == 1).count(), 1);
    }
    #[test]
    fn reject_clipping_and_incomplete_and_bad_sequence() {
        let mut p = Page::new(94, 1, 8, true).unwrap();
        assert!(p.raster().is_err());
        let mut row = vec![255; 94];
        row[0] = 0;
        p.line(0, &row).unwrap();
        assert!(p.raster().is_err());
        assert!(p.line(0, &row).is_err());
        assert!(Page::new(usize::MAX, usize::MAX, 8, true).is_err());
    }
}
