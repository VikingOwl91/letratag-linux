//! Connect-only comparison of the BlueZ and btleplug BLE backends.
//!
//! Records adapter/device identity, the GATT table, selected characteristics,
//! write mode, the computed reference payload hash and write plan, one
//! bounded status attempt, and connect/disconnect behavior for each backend.
//! **Never writes to the TX characteristic; no print payload is transmitted.**
//!
//! Run (from the repository root):
//!   cargo run -p letratag-printer --features ble,btleplug \
//!     --example backend-compare -- --address 90:E5:B1:49:DA:AE

#[cfg(all(feature = "ble", feature = "btleplug"))]
mod imp {
    use letratag_protocol::{Job, Profile};
    use letratag_render::{RenderOptions, render_png};
    use letratag_transport::{BluezTransport, BtleplugTransport, ChunkMode, Transport};
    use std::time::Instant;

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

    fn hex(bytes: &[u8]) -> String {
        bytes
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<Vec<_>>()
            .join(" ")
    }

    pub fn main() -> Result<(), Box<dyn std::error::Error>> {
        let mut address: Option<String> = None;
        let mut png = "test-output/hello-linux.png".to_string();
        let mut args = std::env::args().skip(1);
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--address" => address = Some(args.next().ok_or("--address requires a value")?),
                "--png" => png = args.next().ok_or("--png requires a value")?,
                other => return Err(format!("unknown argument: {other}").into()),
            }
        }
        let address = address.ok_or("--address <MAC> is required")?;

        // 1. Reference payload: identical for both backends by construction
        //    (encoder output depends only on raster, profile and write limit).
        let png_bytes = std::fs::read(&png)?;
        let raster = render_png(
            &png_bytes,
            RenderOptions {
                width: 400,
                ..Default::default()
            },
        )?;
        let job = Job::new(&raster, Profile::Legacy)?;
        let body = job.body();
        let writes = job.writes(244)?;
        let mut wire = Vec::new();
        for write in &writes {
            wire.extend_from_slice(&(write.len() as u16).to_le_bytes());
            wire.extend_from_slice(write);
        }
        let sizes: Vec<usize> = writes.iter().map(|w| w.len()).collect();
        println!("=== reference payload (HELLO LINUX, legacy, cap 244) ===");
        println!(
            "frame: {} x 32; body {} bytes; {} writes {sizes:?}",
            raster.width(),
            body.len(),
            writes.len()
        );
        println!("body sha256: {}", sha256(body));
        println!("wire sha256: {}", sha256(&wire));
        println!(
            "expected body sha256: 58272474123e691ac5d206d11cc91c677efc1b5119a346bb1c7451514d069c53"
        );
        println!(
            "expected wire sha256: 357a6b8dad5a11ca68a39b9825cee3d91551c195f69d7065e07cef0df0eeb98b"
        );
        println!();

        // 2. Per-backend connect-only comparison. No TX writes, no print.
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        rt.block_on(async {
            for backend in ["bluez", "btleplug"] {
                println!("=== backend: {backend} ===");
                let start = Instant::now();
                if backend == "bluez" {
                    let mut transport = BluezTransport::new(&address)?
                        .with_chunk_mode(ChunkMode::Auto)
                        .with_printer_cap(Some(244));
                    match transport.connect().await {
                        Ok(usable) => {
                            println!(
                                "connected in {:?}; usable write size {usable}",
                                start.elapsed()
                            );
                            println!("{}", transport.parameters().describe());
                            if let Some(table) = transport.describe_gatt().await {
                                println!("{table}");
                            }
                            let status_start = Instant::now();
                            let status = transport.status().await;
                            println!(
                                "status attempt after {:?}: {}",
                                status_start.elapsed(),
                                status
                                    .map(|v| hex(&v))
                                    .unwrap_or_else(|e| format!("error: {e}"))
                            );
                            let disconnect_start = Instant::now();
                            let result = transport.disconnect().await;
                            println!("disconnect {result:?} in {:?}", disconnect_start.elapsed());
                        }
                        Err(error) => {
                            println!("connect failed after {:?}: {error}", start.elapsed())
                        }
                    }
                } else {
                    let mut transport = BtleplugTransport::new(&address)?
                        .with_chunk_mode(ChunkMode::Auto)
                        .with_printer_cap(Some(244));
                    match transport.connect().await {
                        Ok(usable) => {
                            println!(
                                "connected in {:?}; usable write size {usable}",
                                start.elapsed()
                            );
                            println!("{}", transport.parameters().describe());
                            if let Some(table) = transport.describe_gatt() {
                                println!("{table}");
                            }
                            let status_start = Instant::now();
                            let status = transport.status().await;
                            println!(
                                "status attempt after {:?}: {}",
                                status_start.elapsed(),
                                status
                                    .map(|v| hex(&v))
                                    .unwrap_or_else(|e| format!("error: {e}"))
                            );
                            let disconnect_start = Instant::now();
                            let result = transport.disconnect().await;
                            println!("disconnect {result:?} in {:?}", disconnect_start.elapsed());
                        }
                        Err(error) => {
                            println!("connect failed after {:?}: {error}", start.elapsed())
                        }
                    }
                }
                println!();
            }
            Ok::<(), Box<dyn std::error::Error>>(())
        })?;
        println!("comparison complete; no print payload was transmitted");
        Ok(())
    }
}

#[cfg(all(feature = "ble", feature = "btleplug"))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    imp::main()
}

#[cfg(not(all(feature = "ble", feature = "btleplug")))]
fn main() {
    eprintln!("backend-compare requires both the ble and btleplug features");
    std::process::exit(2);
}
