use clap::{Parser, Subcommand, ValueEnum};
use letratag_printer::{document, engine::Engine};
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

#[derive(Parser)]
#[command(
    about = "Native LetraTag development driver. Hardware not yet validated.",
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
    },
    /// Local PAPPL endpoint. Always mock-only until hardware acceptance.
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
    },
    /// Print the CUPS integration settings and setup instructions.
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
    #[cfg(feature = "ble")]
    HardwareSend {
        input: PathBuf,
        #[arg(long)]
        address: String,
        #[arg(long)]
        state: PathBuf,
        #[arg(long)]
        acknowledge_unverified_hardware: bool,
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
        /// Print-chunk value size cap in bytes (default 20, ATT23-safe).
        /// Calibration only; larger writes stay disabled until measured.
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
) -> Result<letratag_protocol::Raster, Box<dyn std::error::Error>> {
    if pdf {
        Ok(document::pdf(input)?)
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
        } => {
            let raster = load(&input, pdf, columns, dither)?;
            use std::io::Write;
            use std::os::unix::fs::OpenOptionsExt;
            let mut f = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(output)?;
            f.write_all(&letratag_render::preview_png(&raster)?)?;
            println!(
                "preview: {} columns x 32 protocol rows, geometry uncalibrated",
                raster.width()
            );
        }
        Commands::Mock {
            input,
            state,
            pdf,
            columns,
            capture,
        } => {
            let raster = load(&input, pdf, columns, false)?;
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
        } => {
            let app = letratag_printer::ffi::App::new(Engine::new(state, capture)?)?;
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
        #[cfg(feature = "ble")]
        Commands::HardwareSend {
            input,
            address,
            state,
            acknowledge_unverified_hardware,
            columns,
            diagnostics,
            profile,
            set_cassette_type,
            max_write,
            linger_seconds,
        } => {
            if !acknowledge_unverified_hardware {
                return Err("physical sends require explicit acknowledgment; first read docs/hardware-acceptance.md".into());
            }
            let raster = load(&input, false, columns, false)?;
            let mut transport =
                letratag_transport::BluezTransport::new(&address)?.with_max_write(max_write);
            let mut engine = Engine::new(state, false)?.with_diagnostics(diagnostics);
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
            rt.block_on(engine.print(&raster, profile.into(), &mut transport, true))?;
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
