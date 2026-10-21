use std::path::Path;

fn artwork_matches(width: usize, points: &[(usize, usize)]) -> bool {
    if !matches!(width, 315 | 400)
        || points.is_empty()
        || points.iter().any(|&(x, y)| x >= width || y >= 32)
    {
        return false;
    }
    let bounds = (
        points.iter().map(|p| p.0).min().unwrap(),
        points.iter().map(|p| p.1).min().unwrap(),
        points.iter().map(|p| p.0).max().unwrap(),
        points.iter().map(|p| p.1).max().unwrap(),
    );
    // The fixture's PDF rectangle is x20..120/y15..19 pt at 160 dpi.
    // Its full-page rows33..41 map into wire rows11..19; antialiasing
    // can change the four corner pixels, but not its edges or interior.
    if bounds != (44, 11, 266, 19) {
        return false;
    }
    let mut mask = vec![false; width * 32];
    for &(x, y) in points {
        mask[y * width + x] = true;
    }
    for y in 0..32 {
        for x in 0..width {
            if matches!(x, 44 | 266) && matches!(y, 11 | 19) {
                continue;
            }
            let expected = (44..=266).contains(&x) && (11..=19).contains(&y);
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
            315 => short.push(bytes),
            400 => long.push(bytes),
            _ => panic!("unexpected width {width}"),
        }
    }
    assert_eq!(short.len(), 3);
    assert_eq!(long.len(), 1);
    assert!(short.iter().all(|r| r == &short[0]));
    assert_eq!(&long[0][..315 * 4], &short[0]);
    assert!(long[0][315 * 4..].iter().all(|b| *b == 0));
    println!("PASS: 4 direct-IPP mock captures, identical artwork and 85 additional blank columns");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rectangle() -> Vec<(usize, usize)> {
        (11..=19)
            .flat_map(|y| (44..=266).map(move |x| (x, y)))
            .collect()
    }

    #[test]
    fn accepts_the_independent_rectangle_oracle_at_both_lengths() {
        assert!(artwork_matches(315, &rectangle()));
        assert!(artwork_matches(400, &rectangle()));
    }

    #[test]
    fn rejects_common_shrink_shift_and_mirroring() {
        let source = rectangle();
        for damaged in [
            source
                .iter()
                .copied()
                .filter(|&(x, _)| x <= 200)
                .collect::<Vec<_>>(),
            source.iter().map(|&(x, y)| (x + 1, y)).collect(),
            source.iter().map(|&(x, y)| (314 - x, y)).collect(),
            source.iter().map(|&(x, y)| (x, 31 - y)).collect(),
        ] {
            assert!(!artwork_matches(315, &damaged));
        }
    }

    #[test]
    fn rejects_missing_interior_pixels_and_extra_ink() {
        let mut hole = rectangle();
        hole.retain(|&p| p != (155, 15));
        assert!(!artwork_matches(315, &hole));
        let mut extra = rectangle();
        extra.push((280, 16));
        assert!(!artwork_matches(315, &extra));
    }
}
