use clap::{Parser, Subcommand, ValueEnum};
use letratag_printer::{document, engine::Engine, geometry};
use letratag_protocol::Profile;
use letratag_transport::MockTransport;
use std::path::PathBuf;

#[derive(ValueEnum, Clone, Copy, Debug, Default, PartialEq, Eq)]
enum ProfileChoice {
    /// S2 profile: copies directive, 0x81 raster mode, cut-at-end command.
    #[default]
    Modern,
    /// S1 profile: no copies directive, 0x01 raster mode, form feed.
    Legacy,
}

impl From<ProfileChoice> for Profile {
    fn from(choice: ProfileChoice) -> Self {
        match choice {
            ProfileChoice::Modern => Profile::Modern,
            ProfileChoice::Legacy => Profile::Legacy,
        }
    }
}

#[derive(ValueEnum, Clone, Copy, Debug, Default, PartialEq, Eq)]
enum TransportChoice {
    /// Bounded in-process mock transport (safe default, no Bluetooth).
    #[default]
    Mock,
    /// Real hardware over BlueZ (requires --address; never falls back).
    Bluez,
    /// Real hardware over btleplug (requires --address; never falls back).
    Btleplug,
}

/// Backend selection for the explicit one-shot hardware send.
#[derive(ValueEnum, Clone, Copy, Debug, Default, PartialEq, Eq)]
enum SendTransportChoice {
    /// Real hardware over the existing BlueZ transport (default).
    #[default]
    Bluez,
    /// Real hardware over btleplug (alternative backend for comparison).
    Btleplug,
}

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
enum ChunkModeChoice {
    /// Largest size permitted by the transport and a hardware-validated
    /// printer capability; conservative 20 bytes when none is recorded.
    Auto,
    /// Always the conservative 20-byte limit, or smaller if the transport
    /// requires it.
    Safe,
    /// Explicitly configured size (requires --max-write). Values above the
    /// validated capability are experimental.
    Manual,
}

/// Resolve --chunk-mode/--max-write into the transport chunk mode.
/// --max-write alone implies manual; combining it with auto or safe is a
/// configuration error, and manual without --max-write is incomplete.
#[cfg(any(feature = "pappl", feature = "ble"))]
fn resolve_chunk_mode(
    mode: Option<ChunkModeChoice>,
    max_write: Option<usize>,
) -> Result<letratag_transport::ChunkMode, String> {
    use letratag_transport::ChunkMode;
    let resolved = match (mode, max_write) {
        (None, None) => ChunkMode::Auto,
        (None, Some(value)) => ChunkMode::Manual(value),
        (Some(ChunkModeChoice::Auto), None) => ChunkMode::Auto,
        (Some(ChunkModeChoice::Safe), None) => ChunkMode::Safe,
        (Some(ChunkModeChoice::Manual), Some(value)) => ChunkMode::Manual(value),
        (Some(ChunkModeChoice::Manual), None) => {
            return Err("--chunk-mode manual requires --max-write <bytes>".into());
        }
        (Some(_), Some(_)) => {
            return Err("--max-write requires --chunk-mode manual".into());
        }
    };
    if let ChunkMode::Manual(value) = resolved
        && !(9..=512).contains(&value)
    {
        return Err(format!(
            "--max-write {value} is outside the protocol bounds 9..=512"
        ));
    }
    Ok(resolved)
}

#[derive(Parser)]
#[command(
    about = "Native LetraTag 200B driver. First physical print operator-confirmed 2026-10-20; completion policy stays conservative.",
    version
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Produce a full 32-row wire-raster PNG, without printing.
    Preview {
        input: PathBuf,
        output: PathBuf,
        #[arg(long, default_value_t = 400)]
        columns: usize,
        #[arg(long)]
        pdf: bool,
        #[arg(long)]
        dither: bool,
        /// Calibration profile override: feed=<dpi>,cross=<dpi> (default:
        /// the named LT200B 2026-10-21 empirical profile).
        #[arg(long)]
        calibration: Option<String>,
    },
    /// Complete a validated job using the mock transport and a private journal.
    Mock {
        input: PathBuf,
        #[arg(long)]
        state: PathBuf,
        #[arg(long)]
        pdf: bool,
        #[arg(long, default_value_t = 400)]
        columns: usize,
        #[arg(long)]
        capture: bool,
        /// Calibration profile override: feed=<dpi>,cross=<dpi> (default:
        /// the named LT200B 2026-10-21 empirical profile).
        #[arg(long)]
        calibration: Option<String>,
    },
    /// Local PAPPL endpoint. Mock transport by default; real hardware with
    /// --transport bluez or btleplug (explicit operator choice, no implicit
    /// fallback).
    #[cfg(feature = "pappl")]
    Serve {
        #[arg(long)]
        state: PathBuf,
        #[arg(long)]
        spool: PathBuf,
        #[arg(long, default_value_t = 8631)]
        port: u16,
        #[arg(long)]
        capture: bool,
        /// Transport for submitted jobs.
        #[arg(long, value_enum, default_value_t = TransportChoice::Mock)]
        transport: TransportChoice,
        /// Bluetooth address of the printer (required with --transport
        /// bluez or btleplug).
        #[arg(long)]
        address: Option<String>,
        /// Wire profile. Explicit operator choice; never switched automatically.
        #[arg(long, value_enum, default_value_t = ProfileChoice::Modern)]
        profile: ProfileChoice,
        /// How the BLE print-chunk write size is selected (default: auto).
        #[arg(long, value_enum)]
        chunk_mode: Option<ChunkModeChoice>,
        /// Maximum GATT write-value size in bytes; implies --chunk-mode
        /// manual. Values above the validated capability are experimental.
        #[arg(long)]
        max_write: Option<usize>,
        /// Content-free protocol diagnostics on stderr (transport parameters,
        /// write progress, status bytes, timing). Never logs print contents.
        #[arg(long)]
        diagnostics: bool,
        /// Calibration profile override: feed=<dpi>,cross=<dpi> (default:
        /// the named LT200B 2026-10-21 empirical profile).
        #[arg(long)]
        calibration: Option<String>,
    },
    /// Show Direct IPP guidance and Experimental CUPS settings (informational).
    CupsSettings {
        /// Local PAPPL endpoint port.
        #[arg(long, default_value_t = 8631)]
        port: u16,
    },
    /// Read-only hardware information extraction: full GATT dump, state
    /// reads, RX/AUX notification watch. Never sends a print job.
    #[cfg(feature = "ble")]
    HardwareInfo {
        #[arg(long)]
        address: String,
        /// Seconds to watch RX/AUX notifications while idle.
        #[arg(long, default_value_t = 15)]
        watch: u64,
        /// Also send the set-cassette-type short command (ESC M) and re-read.
        #[arg(long)]
        set_cassette_type: Option<u8>,
    },
    /// Explicit experimental physical send. Always leaves an uncertain journal.
    #[cfg(any(feature = "ble", feature = "btleplug"))]
    HardwareSend {
        input: PathBuf,
        #[arg(long)]
        address: String,
        #[arg(long)]
        state: PathBuf,
        #[arg(long)]
        acknowledge_unverified_hardware: bool,
        /// BLE backend (default: bluez, the established transport).
        #[arg(long, value_enum, default_value_t = SendTransportChoice::Bluez)]
        transport: SendTransportChoice,
        #[arg(long, default_value_t = 40)]
        columns: usize,
        /// Content-free protocol diagnostics on stderr (status bytes, timing).
        #[arg(long)]
        diagnostics: bool,
        /// Wire profile. Explicit operator choice; never switched automatically.
        #[arg(long, value_enum, default_value_t = ProfileChoice::Modern)]
        profile: ProfileChoice,
        /// Send the set-cassette-type short command (ESC M) to the auxiliary
        /// characteristic before the print job. 1=6mm, 2=9mm, 3=12mm,
        /// 4=19mm, 5=24mm. Best effort; the job runs regardless.
        #[arg(long)]
        set_cassette_type: Option<u8>,
        /// How the print-chunk write size is selected (default: auto).
        #[arg(long, value_enum)]
        chunk_mode: Option<ChunkModeChoice>,
        /// Maximum GATT write-value size in bytes; implies --chunk-mode
        /// manual. Must be within 9..=512 and the negotiated transport
        /// limit. Values above the validated capability are experimental.
        #[arg(long)]
        max_write: Option<usize>,
        /// Hardware calibration linger in seconds: after the status phase,
        /// keep the connection open and drain notifications before
        /// disconnecting. Lets a physically in-progress print finish.
        #[arg(long)]
        linger_seconds: Option<u64>,
    },
}

fn load(
    input: &std::path::Path,
    pdf: bool,
    columns: usize,
    dither: bool,
    calibration: &geometry::CalibrationProfile,
) -> Result<letratag_protocol::Raster, Box<dyn std::error::Error>> {
    if pdf {
        Ok(document::pdf_with_calibration(input, calibration)?)
    } else {
        Ok(letratag_render::render_png(
            &document::bounded_read(input)?,
            letratag_render::RenderOptions {
                width: columns,
                dither,
                ..Default::default()
            },
        )?)
    }
}

/// Resolve an optional `--calibration feed=<dpi>,cross=<dpi>` override.
fn resolve_calibration(
    override_text: Option<String>,
) -> Result<geometry::CalibrationProfile, Box<dyn std::error::Error>> {
    match override_text {
        Some(text) => Ok(geometry::parse_calibration_override(&text)?),
        None => Ok(geometry::DEFAULT_CALIBRATION),
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    // Applied only to this process, not the user's shell or any system daemon.
    let limit = libc::rlimit {
        rlim_cur: document::MAX_FILE,
        rlim_max: document::MAX_FILE,
    };
    if unsafe { libc::setrlimit(libc::RLIMIT_FSIZE, &limit) } != 0 {
        return Err("unable to establish spool file limit".into());
    }
    unsafe {
        libc::signal(libc::SIGXFSZ, libc::SIG_IGN);
    }
    std::panic::set_hook(Box::new(|_| {
        eprintln!("internal panic contained; job outcome requires inspection")
    }));
    match Cli::parse().command {
        Commands::Preview {
            input,
            output,
            columns,
            pdf,
            dither,
            calibration,
        } => {
            let calibration = resolve_calibration(calibration)?;
            let raster = load(&input, pdf, columns, dither, &calibration)?;
            use std::io::Write;
            use std::os::unix::fs::OpenOptionsExt;
            let mut f = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(output)?;
            f.write_all(&letratag_render::preview_png(&raster)?)?;
            println!(
                "preview: {} columns x 32 protocol rows, {} dpi logical layout, calibration feed {:.0} / cross {:.0} dpi",
                raster.width(),
                geometry::LOGICAL_DPI as u32,
                calibration.feed_dpi,
                calibration.cross_dpi
            );
        }
        Commands::Mock {
            input,
            state,
            pdf,
            columns,
            capture,
            calibration,
        } => {
            let calibration = resolve_calibration(calibration)?;
            let raster = load(&input, pdf, columns, false, &calibration)?;
            let mut engine = Engine::new(state, capture)?;
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()?;
            rt.block_on(engine.print(
                &raster,
                Profile::Modern,
                &mut MockTransport::default(),
                false,
            ))?;
            println!("mock job completed; no Bluetooth communication");
        }
        #[cfg(feature = "pappl")]
        Commands::Serve {
            state,
            spool,
            port,
            capture,
            transport,
            address,
            profile,
            chunk_mode,
            max_write,
            diagnostics,
            calibration,
        } => {
            use letratag_printer::ffi::TransportKind;
            let chunk_mode = resolve_chunk_mode(chunk_mode, max_write)?;
            let calibration = resolve_calibration(calibration)?;
            let engine = Engine::new(state.clone(), capture)?.with_diagnostics(diagnostics);
            let kind = match transport {
                TransportChoice::Mock => TransportKind::Mock,
                TransportChoice::Bluez => {
                    let address = address.ok_or("--transport bluez requires --address <MAC>")?;
                    let printer_cap = letratag_printer::caps::lookup(&state, &address)
                        .map_err(|e| e.to_string())?;
                    TransportKind::Bluez {
                        address,
                        chunk_mode,
                        printer_cap,
                    }
                }
                TransportChoice::Btleplug => {
                    let address = address.ok_or("--transport btleplug requires --address <MAC>")?;
                    let printer_cap = letratag_printer::caps::lookup(&state, &address)
                        .map_err(|e| e.to_string())?;
                    TransportKind::Btleplug {
                        address,
                        chunk_mode,
                        printer_cap,
                    }
                }
            };
            if let Some(message) = kind.unsupported() {
                return Err(message.into());
            }
            match &kind {
                TransportKind::Mock => {
                    eprintln!("serving mock transport on 127.0.0.1:{port} (no Bluetooth)");
                }
                TransportKind::Bluez {
                    chunk_mode,
                    printer_cap,
                    ..
                }
                | TransportKind::Btleplug {
                    chunk_mode,
                    printer_cap,
                    ..
                } => {
                    let cap = printer_cap.map_or("unknown".to_string(), |cap| cap.to_string());
                    eprintln!(
                        "serving {:?} transport on 127.0.0.1:{port}; chunk mode: {chunk_mode:?}, validated printer cap: {cap}",
                        kind.backend_name()
                    );
                }
            }
            let app = letratag_printer::ffi::App::new(engine, kind, profile.into(), calibration)?;
            letratag_printer::ffi::serve(app, &spool, port).map_err(std::io::Error::other)?;
        }
        Commands::CupsSettings { port } => {
            print!("{}", letratag_printer::cups::settings_report(port));
        }
        #[cfg(feature = "ble")]
        Commands::HardwareInfo {
            address,
            watch,
            set_cassette_type,
        } => {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()?;
            rt.block_on(letratag_printer::hardware_info::run(
                &address,
                std::time::Duration::from_secs(watch),
                set_cassette_type,
            ))?;
        }
        #[cfg(any(feature = "ble", feature = "btleplug"))]
        Commands::HardwareSend {
            input,
            address,
            state,
            acknowledge_unverified_hardware,
            transport: send_transport,
            columns,
            diagnostics,
            profile,
            set_cassette_type,
            chunk_mode,
            max_write,
            linger_seconds,
        } => {
            if !acknowledge_unverified_hardware {
                return Err("physical sends require explicit acknowledgment; first read docs/hardware-acceptance.md".into());
            }
            let chunk_mode = resolve_chunk_mode(chunk_mode, max_write)?;
            let raster = load(
                &input,
                false,
                columns,
                false,
                &geometry::DEFAULT_CALIBRATION,
            )?;
            let mut engine = Engine::new(state.clone(), false)?.with_diagnostics(diagnostics);
            let printer_cap =
                letratag_printer::caps::lookup(&state, &address).map_err(|e| e.to_string())?;
            let mut transport: Box<dyn letratag_transport::Transport> = match send_transport {
                #[cfg(feature = "ble")]
                SendTransportChoice::Bluez => Box::new(
                    letratag_transport::BluezTransport::new(&address)?
                        .with_chunk_mode(chunk_mode)
                        .with_printer_cap(printer_cap),
                ),
                #[cfg(not(feature = "ble"))]
                SendTransportChoice::Bluez => {
                    return Err(
                        "the ble feature is not compiled in; build with --features ble".into(),
                    );
                }
                #[cfg(feature = "btleplug")]
                SendTransportChoice::Btleplug => Box::new(
                    letratag_transport::BtleplugTransport::new(&address)?
                        .with_chunk_mode(chunk_mode)
                        .with_printer_cap(printer_cap),
                ),
                #[cfg(not(feature = "btleplug"))]
                SendTransportChoice::Btleplug => {
                    return Err(
                        "the btleplug feature is not compiled in; build with --features btleplug"
                            .into(),
                    );
                }
            };
            if let Some(id) = set_cassette_type {
                engine = engine
                    .with_short_command(Some(letratag_protocol::set_cassette_type_write(id)?));
            }
            if let Some(seconds) = linger_seconds {
                engine = engine.with_linger(std::time::Duration::from_secs(seconds));
            }
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()?;
            rt.block_on(engine.print(&raster, profile.into(), transport.as_mut(), true))?;
        }
    }
    Ok(())
}

fn main() {
    if let Err(e) = run() {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
