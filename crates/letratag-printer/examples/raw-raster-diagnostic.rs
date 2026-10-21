//! One-shot RAW raster diagnostic for the DYMO LetraTag 200B.
//!
//! Purpose: measure the physical feed pitch independently of PDF, Poppler,
//! PNG, CUPS, IPP, PAPPL and every document-scaling component. The 1-bit
//! raster is generated deterministically in memory (built-in 5x7 bitmap
//! font + one exactly 160-feed-column reference line), encoded with the
//! existing `letratag-protocol` legacy encoder, and sent over the existing
//! BlueZ transport through the existing engine/journal path. No new wire
//! format and no production rendering change.
//!
//! Exactly one physical transmission per invocation, only with
//! `--acknowledge-unverified-hardware`; the engine never retries.
//! `--dry-run` performs every pre-print verification without Bluetooth I/O.
//!
//! Run through scripts/raw-raster-diagnostic.sh (cargo example, ble feature).

#[cfg(feature = "ble")]
mod imp {
    use std::io::Write;
    use std::path::{Path, PathBuf};

    use letratag_printer::caps;
    use letratag_printer::engine::{Engine, private_directory};
    use letratag_protocol::{HEAD_ROWS, Job, Profile, Raster};
    use letratag_transport::{BluezTransport, ChunkMode};

    const WIDTH: usize = 400;
    const TEXT: &str = "RAW TEST";
    const TEXT_X: usize = 20;
    const TEXT_Y: usize = 3;
    const LINE_ROW: usize = 16;
    const LINE_START: usize = 20;
    const LINE_LEN: usize = 160;
    const TICK_TOP: usize = 14;
    const TICK_BOTTOM: usize = 18;
    /// Hardware-validated GATT write-value cap for the test device
    /// (operator-confirmed 2026-10-20, recorded in printer-caps.conf).
    const MAX_WRITE: usize = 244;

    /// Built-in 5x7 bitmap font; '#' is ink. Covers exactly the diagnostic text.
    fn glyph(c: char) -> Option<[&'static str; 7]> {
        Some(match c {
            'R' => [
                "#### ", "#   #", "#   #", "#### ", "#  # ", "#   #", "#   #",
            ],
            'A' => [
                " ### ", "#   #", "#   #", "#####", "#   #", "#   #", "#   #",
            ],
            'W' => [
                "#   #", "#   #", "#   #", "# # #", "# # #", "## ##", "#   #",
            ],
            'T' => [
                "#####", "  #  ", "  #  ", "  #  ", "  #  ", "  #  ", "  #  ",
            ],
            'E' => [
                "#####", "#    ", "#    ", "#### ", "#    ", "#    ", "#####",
            ],
            'S' => [
                " ####", "#    ", "#    ", " ### ", "    #", "    #", "#### ",
            ],
            ' ' => [
                "     ", "     ", "     ", "     ", "     ", "     ", "     ",
            ],
            _ => return None,
        })
    }

    fn build_raster() -> Raster {
        let mut pixels = vec![0u8; WIDTH * HEAD_ROWS];
        for (ci, c) in TEXT.chars().enumerate() {
            let glyph = glyph(c).expect("built-in font covers the diagnostic text");
            for (row, bits) in glyph.iter().enumerate() {
                for (col, bit) in bits.chars().enumerate() {
                    if bit == '#' {
                        pixels[(TEXT_Y + row) * WIDTH + TEXT_X + ci * 6 + col] = 1;
                    }
                }
            }
        }
        for x in LINE_START..LINE_START + LINE_LEN {
            pixels[LINE_ROW * WIDTH + x] = 1;
        }
        for y in TICK_TOP..=TICK_BOTTOM {
            pixels[y * WIDTH + LINE_START] = 1;
            pixels[y * WIDTH + LINE_START + LINE_LEN - 1] = 1;
        }
        Raster::new(WIDTH, pixels).expect("in-memory raster is valid")
    }

    /// Compact SHA-256 (FIPS 180-4) so the diagnostic records payload hashes
    /// without adding a dependency.
    fn sha256(data: &[u8]) -> String {
        const K: [u32; 64] = [
            0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
            0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
            0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
            0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
            0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
            0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
            0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
            0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
            0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
            0xc67178f2,
        ];
        let mut h: [u32; 8] = [
            0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
            0x5be0cd19,
        ];
        let bitlen = (data.len() as u64).wrapping_mul(8);
        let mut msg = data.to_vec();
        msg.push(0x80);
        while msg.len() % 64 != 56 {
            msg.push(0);
        }
        msg.extend_from_slice(&bitlen.to_be_bytes());
        for chunk in msg.as_chunks::<64>().0 {
            let mut w = [0u32; 64];
            for i in 0..16 {
                w[i] = u32::from_be_bytes(chunk[i * 4..i * 4 + 4].try_into().unwrap());
            }
            for i in 16..64 {
                let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
                let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
                w[i] = w[i - 16]
                    .wrapping_add(s0)
                    .wrapping_add(w[i - 7])
                    .wrapping_add(s1);
            }
            let (mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh) =
                (h[0], h[1], h[2], h[3], h[4], h[5], h[6], h[7]);
            for i in 0..64 {
                let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
                let ch = (e & f) ^ ((!e) & g);
                let t1 = hh
                    .wrapping_add(s1)
                    .wrapping_add(ch)
                    .wrapping_add(K[i])
                    .wrapping_add(w[i]);
                let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
                let maj = (a & b) ^ (a & c) ^ (b & c);
                let t2 = s0.wrapping_add(maj);
                hh = g;
                g = f;
                f = e;
                e = d.wrapping_add(t1);
                d = c;
                c = b;
                b = a;
                a = t1.wrapping_add(t2);
            }
            h[0] = h[0].wrapping_add(a);
            h[1] = h[1].wrapping_add(b);
            h[2] = h[2].wrapping_add(c);
            h[3] = h[3].wrapping_add(d);
            h[4] = h[4].wrapping_add(e);
            h[5] = h[5].wrapping_add(f);
            h[6] = h[6].wrapping_add(g);
            h[7] = h[7].wrapping_add(hh);
        }
        h.iter().map(|x| format!("{x:08x}")).collect()
    }

    fn write_private(path: &Path, data: &[u8]) -> std::io::Result<()> {
        use std::os::unix::fs::OpenOptionsExt;
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(path)?;
        file.write_all(data)
    }

    /// Decode the length-prefixed capture back into a raster and verify the
    /// diagnostic geometry independently of the encoder path.
    fn verify_capture(capture: &[u8], source: &Raster) -> Result<String, String> {
        let mut cursor = 0usize;
        let mut writes: Vec<&[u8]> = Vec::new();
        while cursor < capture.len() {
            if cursor + 2 > capture.len() {
                return Err("truncated capture length".into());
            }
            let size = u16::from_le_bytes([capture[cursor], capture[cursor + 1]]) as usize;
            cursor += 2;
            if size == 0 || cursor + size > capture.len() {
                return Err("truncated or empty capture write".into());
            }
            writes.push(&capture[cursor..cursor + size]);
            cursor += size;
        }
        if writes.len() < 2 || writes[0].len() != 9 || writes[0][..4] != [0xff, 0xf0, 0x12, 0x34] {
            return Err("invalid capture header".into());
        }
        let mut body = Vec::new();
        for (i, write) in writes[1..].iter().enumerate() {
            let expected_index = i + usize::from(i >= 27);
            if write[0] as usize != expected_index {
                return Err("invalid chunk index".into());
            }
            let end = if i == writes.len() - 2 {
                if write[write.len() - 2..] != [0x12, 0x34] {
                    return Err("missing final magic".into());
                }
                write.len() - 2
            } else {
                write.len()
            };
            body.extend_from_slice(&write[1..end]);
        }
        if body.len() != u32::from_le_bytes(writes[0][4..8].try_into().unwrap()) as usize {
            return Err("body length mismatch".into());
        }
        if body[..6] != [0x1b, 0x73, 0x9a, 0x02, 0x00, 0x00] || body[6..8] != [0x1b, 0x44] {
            return Err("unexpected legacy command preamble".into());
        }
        if body[8] != 0x01 || body[9] != 0x02 {
            return Err("unexpected ESC D mode/alignment".into());
        }
        let width = u32::from_le_bytes(body[10..14].try_into().unwrap()) as usize;
        let height = u32::from_le_bytes(body[14..18].try_into().unwrap()) as usize;
        if height != HEAD_ROWS || width != WIDTH {
            return Err(format!("unexpected ESC D geometry {width}x{height}"));
        }
        let decoded = Raster::from_packed_columns(width, &body[18..18 + width * 4])
            .map_err(|e| format!("raster decode failed: {e}"))?;
        if decoded != *source {
            return Err("decoded raster differs from the in-memory source".into());
        }
        let line: Vec<usize> = (0..width).filter(|&x| decoded.get(x, LINE_ROW)).collect();
        if line.len() != LINE_LEN
            || line.first() != Some(&LINE_START)
            || line.last() != Some(&(LINE_START + LINE_LEN - 1))
        {
            return Err(format!(
                "reference line is not exactly {LINE_LEN} columns at {LINE_START}..={}",
                LINE_START + LINE_LEN - 1
            ));
        }
        if decoded.get(LINE_START - 1, LINE_ROW) || decoded.get(LINE_START + LINE_LEN, LINE_ROW) {
            return Err("reference line bleeds into neighbouring columns".into());
        }
        let ink_rows: Vec<usize> = (0..HEAD_ROWS)
            .filter(|&y| (0..width).any(|x| decoded.get(x, y)))
            .collect();
        if ink_rows.iter().any(|&y| !(1..=30).contains(&y)) {
            return Err("ink outside the printable rows 1..=30".into());
        }
        let text_ink: usize = (TEXT_Y..TEXT_Y + 7)
            .map(|y| (0..width).filter(|&x| decoded.get(x, y)).count())
            .sum();
        if text_ink == 0 {
            return Err("diagnostic text missing".into());
        }
        Ok(format!(
            "PASS: width {width}, height {height}, decoded raster identical to source; \
         reference line row {LINE_ROW} ink exactly at columns {LINE_START}..={} ({LINE_LEN} feed columns); \
         neighbours white; text ink {text_ink} px; ink rows {}..={}",
            LINE_START + LINE_LEN - 1,
            ink_rows.first().unwrap(),
            ink_rows.last().unwrap()
        ))
    }

    pub fn main() -> Result<(), Box<dyn std::error::Error>> {
        let mut address: Option<String> = None;
        let mut state: Option<PathBuf> = None;
        let mut dry_run = false;
        let mut acknowledged = false;
        let mut args = std::env::args().skip(1);
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--address" => address = Some(args.next().ok_or("--address requires a value")?),
                "--state" => {
                    state = Some(PathBuf::from(
                        args.next().ok_or("--state requires a value")?,
                    ))
                }
                "--dry-run" => dry_run = true,
                "--acknowledge-unverified-hardware" => acknowledged = true,
                other => return Err(format!("unknown argument: {other}").into()),
            }
        }
        let address = address.ok_or("--address <MAC> is required")?;
        let state = state.ok_or("--state <DIR> is required")?;

        // Private state directory (engine requirement: mode 0700).
        private_directory(&state)?;

        // Build and encode entirely in memory; no document pipeline involved.
        let raster = build_raster();
        let job = Job::new(&raster, Profile::Legacy)?;
        let body = job.body().to_vec();
        let writes = job.writes(MAX_WRITE)?;
        let mut capture = Vec::new();
        for write in &writes {
            capture.extend_from_slice(&(write.len() as u16).to_le_bytes());
            capture.extend_from_slice(write);
        }
        let packed = raster.pack_columns();
        let write_lengths: Vec<usize> = writes.iter().map(|w| w.len()).collect();

        let verification = verify_capture(&capture, &raster).map_err(|e| e.to_string())?;

        let mut report = String::new();
        report.push_str("RAW RASTER DIAGNOSTIC (one-shot; exactly one physical print)\n");
        report.push_str(&format!(
            "raster: {WIDTH} x {HEAD_ROWS} row-major 1-bit (feed columns x wire rows)\n"
        ));
        report.push_str(&format!(
            "text: \"{TEXT}\" built-in 5x7 font at x={TEXT_X}..{}, y={TEXT_Y}..=9\n",
            TEXT_X + TEXT.chars().count() * 6 - 2
        ));
        report.push_str(&format!(
            "reference line: y={LINE_ROW}, x={LINE_START}..={} (exactly {LINE_LEN} feed columns); \
         endpoint ticks y={TICK_TOP}..={TICK_BOTTOM} at x={LINE_START} and x={}\n",
            LINE_START + LINE_LEN - 1,
            LINE_START + LINE_LEN - 1
        ));
        report.push_str(&format!(
        "payload: legacy profile, body {} bytes, {} writes {write_lengths:?} at validated cap {MAX_WRITE}\n",
        body.len(),
        writes.len()
    ));
        report.push_str(&format!(
            "esc-d: {}\n",
            body[6..18]
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<Vec<_>>()
                .join(" ")
        ));
        report.push_str(&format!(
            "sha256 raster (packed columns): {}\n",
            sha256(&packed)
        ));
        report.push_str(&format!("sha256 body: {}\n", sha256(&body)));
        report.push_str(&format!(
            "sha256 wire (u16le length-prefixed writes): {}\n",
            sha256(&capture)
        ));
        report.push_str(&format!("decode verification: {verification}\n"));
        report.push_str(
            "components: letratag-protocol encoder + letratag-transport BlueZ + letratag-printer \
         Engine/journal only; no PDF, Poppler, PNG, CUPS, IPP, or PAPPL\n",
        );
        report.push_str(
            "operator measurement guide: ~25.4 mm = 160 dpi feed pitch; ~12.9 mm = 315-316 dpi \
         apparent feed pitch; anything else needs investigation without assuming a mechanism\n",
        );

        write_private(&state.join("raw-test.capture.ble"), &capture)?;
        write_private(&state.join("raw-test.body.bin"), &body)?;
        write_private(&state.join("raw-test.raster.bin"), &packed)?;
        print!("{report}");
        write_private(&state.join("raw-test-evidence.txt"), report.as_bytes())?;
        println!(
            "PRE-TRANSMISSION RECORD COMPLETE (evidence in {})",
            state.display()
        );

        if dry_run {
            println!("DRY RUN: all verification passed; no Bluetooth I/O performed");
            return Ok(());
        }
        if !acknowledged {
            return Err(
                "physical transmission requires --acknowledge-unverified-hardware; read \
             docs/hardware-acceptance.md first"
                    .into(),
            );
        }

        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        let outcome: Result<(), String> = rt.block_on(async {
            let cap = caps::lookup(&state, &address).map_err(|e| e.to_string())?;
            eprintln!(
                "[diag] validated printer cap: {}",
                cap.map_or("none".to_string(), |c| c.to_string())
            );
            let mut transport = BluezTransport::new(&address)
                .map_err(|e| e.to_string())?
                .with_chunk_mode(ChunkMode::Auto)
                .with_printer_cap(cap);
            let mut engine = Engine::new(&state, false)
                .map_err(|e| e.to_string())?
                .with_diagnostics(true);
            engine
                .print(&raster, Profile::Legacy, &mut transport, true)
                .await
                .map_err(|e| e.to_string())
        });
        eprintln!("[diag] outcome {outcome:?}");
        match outcome {
            Ok(()) => println!("engine reported a certain completion; still inspect the tape"),
            Err(e) => {
                println!("engine outcome {e}; inspect the tape and record the journal decision")
            }
        }
        Ok(())
    }
}

#[cfg(feature = "ble")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    imp::main()
}

#[cfg(not(feature = "ble"))]
fn main() {
    eprintln!(
        "raw-raster-diagnostic requires the ble feature; use scripts/raw-raster-diagnostic.sh"
    );
    std::process::exit(2);
}
