use std::path::Path;

/// Expected rectangle bounds for a capture width. The fixture's PDF
/// rectangle is x20..120/y15..19 pt, rendered at the physical calibration
/// (feed 315 / cross 160 DPI) on a 12 mm-tall page. The 50 mm page renders
/// 620x76 px with the rectangle at x87..524; the 63.5 mm page renders
/// 788x76 px and the same pt rectangle lands at x88..524 (one pixel of
/// antialiasing shift from the different page scale). Full-page rows
/// 34..41 map into wire rows 12..19. Antialiasing can change the four
/// corner pixels, but not the edges or interior.
fn expected_bounds(width: usize) -> Option<(usize, usize, usize, usize)> {
    match width {
        620 => Some((87, 12, 524, 19)),
        788 => Some((88, 12, 524, 19)),
        _ => None,
    }
}

fn artwork_matches(width: usize, points: &[(usize, usize)]) -> bool {
    let Some((x0, y0, x1, y1)) = expected_bounds(width) else {
        return false;
    };
    if points.is_empty() || points.iter().any(|&(x, y)| x >= width || y >= 32) {
        return false;
    }
    let bounds = (
        points.iter().map(|p| p.0).min().unwrap(),
        points.iter().map(|p| p.1).min().unwrap(),
        points.iter().map(|p| p.0).max().unwrap(),
        points.iter().map(|p| p.1).max().unwrap(),
    );
    if bounds != (x0, y0, x1, y1) {
        return false;
    }
    let mut mask = vec![false; width * 32];
    for &(x, y) in points {
        mask[y * width + x] = true;
    }
    for y in 0..32 {
        for x in 0..width {
            if (x == x0 || x == x1) && (y == y0 || y == y1) {
                continue;
            }
            let expected = (x0..=x1).contains(&x) && (y0..=y1).contains(&y);
            if mask[y * width + x] != expected {
                return false;
            }
        }
    }
    true
}

fn raster(path: &Path) -> (usize, Vec<u8>) {
    let data = std::fs::read(path).unwrap();
    let mut cursor = 0;
    let mut writes = Vec::new();
    while cursor < data.len() {
        let size = u16::from_le_bytes(data[cursor..cursor + 2].try_into().unwrap()) as usize;
        cursor += 2;
        assert!((1..=512).contains(&size));
        writes.push(&data[cursor..cursor + size]);
        cursor += size;
    }
    let header = writes[0];
    assert_eq!(header.len(), 9);
    assert_eq!(&header[..4], &[255, 240, 18, 52]);
    assert_eq!(
        header[..8].iter().fold(0u8, |a, b| a.wrapping_add(*b)),
        header[8]
    );
    let mut body = Vec::new();
    for (i, write) in writes[1..].iter().enumerate() {
        assert_eq!(write[0] as usize, i + usize::from(i >= 27));
        let end = if i == writes.len() - 2 {
            assert_eq!(&write[write.len() - 2..], &[18, 52]);
            write.len() - 2
        } else {
            write.len()
        };
        body.extend_from_slice(&write[1..end]);
    }
    assert_eq!(
        body.len(),
        u32::from_le_bytes(header[4..8].try_into().unwrap()) as usize
    );
    assert_eq!(&body[..10], &[27, 115, 154, 2, 0, 0, 27, 68, 1, 2]);
    let width = u32::from_le_bytes(body[10..14].try_into().unwrap()) as usize;
    assert_eq!(&body[14..18], &[32, 0, 0, 0]);
    assert_eq!(body.len(), 24 + width * 4);
    assert_eq!(&body[18 + width * 4..], &[27, 69, 27, 65, 27, 81]);
    let packed = body[18..18 + width * 4].to_vec();
    let mut points = Vec::new();
    for x in 0..width {
        let column = packed[x * 4..x * 4 + 4].iter().rev();
        for (group, byte) in column.enumerate() {
            for bit in 0..8 {
                if byte & (128 >> bit) != 0 {
                    points.push((x, group * 8 + bit));
                }
            }
        }
    }
    assert!(!points.is_empty());
    assert!(points.iter().all(|(_, y)| (1..=30).contains(y)));
    assert!(
        artwork_matches(width, &points),
        "unexpected fixture geometry"
    );
    let min_x = points.iter().map(|p| p.0).min().unwrap();
    let max_x = points.iter().map(|p| p.0).max().unwrap();
    let min_y = points.iter().map(|p| p.1).min().unwrap();
    let max_y = points.iter().map(|p| p.1).max().unwrap();
    println!(
        "{}: {width}x32, ink x{min_x}..{max_x} y{min_y}..{max_y}",
        path.display()
    );
    (width, packed)
}

fn main() {
    let mut short = Vec::new();
    let mut long = Vec::new();
    for entry in std::fs::read_dir(std::env::args().nth(1).unwrap()).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_none_or(|e| e != "ble") {
            continue;
        }
        let (width, bytes) = raster(&path);
        match width {
            620 => short.push(bytes),
            788 => long.push(bytes),
            _ => panic!("unexpected width {width}"),
        }
    }
    assert_eq!(short.len(), 3);
    assert_eq!(long.len(), 1);
    // The three 50 mm renders are byte-identical to each other.
    assert!(short.iter().all(|r| r == &short[0]));
    // The 63.5 mm render carries the same artwork, proportionally scaled:
    // the same physical rectangle (40 mm x ~1.27 mm ink extent) at its own
    // oracle bounds, with the additional feed columns blank.
    assert!(long[0][620 * 4..].iter().all(|b| *b == 0));
    println!("PASS: 4 direct-IPP mock captures, proportional artwork and 168 additional blank columns");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rectangle(width: usize) -> Vec<(usize, usize)> {
        let (x0, y0, x1, y1) = expected_bounds(width).unwrap();
        (y0..=y1)
            .flat_map(|y| (x0..=x1).map(move |x| (x, y)))
            .collect()
    }

    #[test]
    fn accepts_the_independent_rectangle_oracle_at_both_lengths() {
        assert!(artwork_matches(620, &rectangle(620)));
        assert!(artwork_matches(788, &rectangle(788)));
    }

    #[test]
    fn rejects_common_shrink_shift_and_mirroring() {
        let source = rectangle(620);
        for damaged in [
            source
                .iter()
                .copied()
                .filter(|&(x, _)| x <= 200)
                .collect::<Vec<_>>(),
            source.iter().map(|&(x, y)| (x + 1, y)).collect(),
            source.iter().map(|&(x, y)| (619 - x, y)).collect(),
            // Mirror around a non-center axis (the rectangle is vertically
            // symmetric, so a center flip would be an identity).
            source.iter().map(|&(x, y)| (x, 24 - y)).collect(),
        ] {
            assert!(!artwork_matches(620, &damaged));
        }
    }

    #[test]
    fn rejects_missing_interior_pixels_and_extra_ink() {
        let mut hole = rectangle(620);
        hole.retain(|&p| p != (300, 15));
        assert!(!artwork_matches(620, &hole));
        let mut extra = rectangle(620);
        extra.push((600, 16));
        assert!(!artwork_matches(620, &extra));
    }

    #[test]
    fn rejects_the_other_lengths_bounds() {
        // The 63.5 mm render's one-pixel-shifted rectangle must not match the
        // 50 mm oracle bounds, and vice versa.
        assert!(!artwork_matches(620, &rectangle(788)));
        assert!(!artwork_matches(788, &rectangle(620)));
    }
}
