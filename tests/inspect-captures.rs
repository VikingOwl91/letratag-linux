use std::path::Path;
fn main() {
    let mut jobs = 0;
    for entry in std::fs::read_dir(&std::env::args().nth(1).unwrap()).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_none_or(|e| e != "ble") {
            continue;
        }
        inspect(&path);
        jobs += 1;
    }
    assert_eq!(jobs, 4);
    println!("validated 4 independently decoded mock packet captures");
}
fn inspect(path: &Path) {
    let data = std::fs::read(path).unwrap();
    let mut cursor = 0;
    let mut writes = Vec::new();
    while cursor < data.len() {
        assert!(cursor + 2 <= data.len());
        let len = u16::from_le_bytes([data[cursor], data[cursor + 1]]) as usize;
        cursor += 2;
        assert!((1..=512).contains(&len));
        assert!(cursor + len <= data.len());
        writes.push(&data[cursor..cursor + len]);
        cursor += len;
    }
    let header = writes[0];
    assert_eq!(header.len(), 9);
    assert_eq!(&header[..4], &[255, 240, 18, 52]);
    assert_eq!(
        header[..8].iter().fold(0u8, |s, b| s.wrapping_add(*b)),
        header[8]
    );
    let length = u32::from_le_bytes(header[4..8].try_into().unwrap()) as usize;
    let mut body = Vec::new();
    for (i, chunk) in writes[1..].iter().enumerate() {
        assert_eq!(chunk[0] as usize, if i >= 27 { i + 1 } else { i });
        let end = if i == writes.len() - 2 {
            assert_eq!(&chunk[chunk.len() - 2..], &[18, 52]);
            chunk.len() - 2
        } else {
            chunk.len()
        };
        body.extend_from_slice(&chunk[1..end]);
    }
    assert_eq!(body.len(), length);
    assert_eq!(
        &body[..13],
        &[27, 115, 154, 2, 0, 0, 27, 35, 1, 27, 68, 129, 2]
    );
    let width = u32::from_le_bytes(body[13..17].try_into().unwrap()) as usize;
    // 50 mm feed at the measured 160 dpi layout: 314.96 px, plus rounding.
    assert!((314..=316).contains(&width));
    assert_eq!(&body[17..21], &[32, 0, 0, 0]);
    assert_eq!(body.len(), width * 4 + 28);
    assert_eq!(&body[21 + width * 4..], &[27, 112, 48, 27, 65, 27, 81]);
    assert!(body[21..21 + width * 4].iter().any(|b| *b != 0));
}
