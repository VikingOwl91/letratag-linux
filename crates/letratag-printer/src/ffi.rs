use crate::{document, engine::Engine, geometry};
use letratag_protocol::{Profile, Raster};
use letratag_transport::{ChunkMode, MockTransport};
use std::ffi::{CString, OsStr, c_char, c_int, c_void};
use std::os::unix::ffi::OsStrExt;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::Path;
use std::sync::Mutex;

/// How submitted jobs reach a printer. Mock is the safe default; Bluez
/// targets real hardware. There is never an implicit fallback from Bluez
/// to mock: a requested Bluez transport either runs as Bluez or fails.
pub enum TransportKind {
    Mock,
    Bluez {
        address: String,
        chunk_mode: ChunkMode,
        /// Hardware-validated write capability recorded for this address.
        printer_cap: Option<usize>,
    },
}

impl TransportKind {
    /// Error message when the requested transport is not compiled in.
    #[cfg(feature = "ble")]
    pub fn unsupported(&self) -> Option<&'static str> {
        let _ = self;
        None
    }

    #[cfg(not(feature = "ble"))]
    pub fn unsupported(&self) -> Option<&'static str> {
        match self {
            TransportKind::Mock => None,
            TransportKind::Bluez { .. } => {
                Some("the ble feature is not compiled in; build with --features pappl,ble")
            }
        }
    }
}

pub struct App {
    engine: Mutex<Engine>,
    runtime: tokio::runtime::Runtime,
    transport: TransportKind,
    profile: Profile,
}

impl App {
    pub fn new(
        engine: Engine,
        transport: TransportKind,
        profile: Profile,
    ) -> std::io::Result<Self> {
        Ok(Self {
            engine: Mutex::new(engine),
            runtime: tokio::runtime::Builder::new_multi_thread()
                .worker_threads(1)
                .enable_all()
                .build()?,
            transport,
            profile,
        })
    }
    fn submit(&self, raster: &Raster) -> c_int {
        let Ok(mut engine) = self.engine.lock() else {
            return 1;
        };
        // The engine mutex plus the journal lock serialize jobs; one owner
        // at a time, no concurrent transmissions to the same printer.
        match &self.transport {
            TransportKind::Mock => i32::from(
                self.runtime
                    .block_on(engine.print(
                        raster,
                        self.profile,
                        &mut MockTransport::default(),
                        false,
                    ))
                    .is_err(),
            ),
            TransportKind::Bluez {
                address,
                chunk_mode,
                printer_cap,
            } => self.submit_bluez(&mut engine, raster, address, *chunk_mode, *printer_cap),
        }
    }

    #[cfg(feature = "ble")]
    fn submit_bluez(
        &self,
        engine: &mut Engine,
        raster: &Raster,
        address: &str,
        chunk_mode: ChunkMode,
        printer_cap: Option<usize>,
    ) -> c_int {
        let Ok(transport) = letratag_transport::BluezTransport::new(address) else {
            return 1;
        };
        let mut transport = transport
            .with_chunk_mode(chunk_mode)
            .with_printer_cap(printer_cap);
        // The hardware path: bounded timeouts, no retries, uncertain
        // outcome persisted in the journal (see engine::JobState).
        i32::from(
            self.runtime
                .block_on(engine.print(raster, self.profile, &mut transport, true))
                .is_err(),
        )
    }

    #[cfg(not(feature = "ble"))]
    fn submit_bluez(
        &self,
        _engine: &mut Engine,
        _raster: &Raster,
        _address: &str,
        _chunk_mode: ChunkMode,
        _printer_cap: Option<usize>,
    ) -> c_int {
        1
    }
}

#[repr(C)]
struct Config {
    dpi: u32,
    media_width: u32,
    min_length: u32,
    max_length: u32,
    default_length: u32,
    max_job_bytes: usize,
    model: *const c_char,
}

unsafe extern "C" {
    fn lt_serve(
        app: *mut c_void,
        config: *const Config,
        spool: *const c_char,
        port: c_int,
    ) -> c_int;
}

pub fn serve(app: App, spool: &Path, port: u16) -> Result<(), String> {
    if port < 1024 {
        return Err("use an unprivileged local port".into());
    }
    crate::engine::private_directory(spool).map_err(|_| "spool directory must be private")?;
    let spool = CString::new(spool.as_os_str().as_bytes()).map_err(|_| "invalid spool path")?;
    let model = match &app.transport {
        TransportKind::Mock => CString::new(format!(
            "DYMO LetraTag 200B MOCK - {}-dpi layout",
            geometry::RESOLUTION_DPI as u32
        ))
        .unwrap(),
        TransportKind::Bluez { .. } => {
            CString::new("DYMO LetraTag 200B (BLE, experimental)").unwrap()
        }
    };
    let config = Config {
        dpi: geometry::RESOLUTION_DPI as u32,
        media_width: geometry::MEDIA_WIDTH,
        min_length: geometry::MEDIA_MIN_LENGTH,
        max_length: geometry::MEDIA_MAX_LENGTH,
        default_length: geometry::MEDIA_DEFAULT_LENGTH,
        max_job_bytes: document::MAX_FILE as usize,
        model: model.as_ptr(),
    };
    let app = Box::new(app);
    // The C server exits the process after its run loop, without freeing borrowed state.
    // Only startup failures before worker creation can return from lt_serve.
    let result = unsafe {
        lt_serve(
            (&*app as *const App).cast_mut().cast(),
            &config,
            spool.as_ptr(),
            c_int::from(port),
        )
    };
    if result == 0 {
        Ok(())
    } else {
        Err(format!("PAPPL integration failed ({result})"))
    }
}

fn boundary<T>(failure: T, f: impl FnOnce() -> T) -> T {
    catch_unwind(AssertUnwindSafe(f)).unwrap_or(failure)
}

/// # Safety
/// app is live; path is a borrowed buffer of len bytes, supplied only by PAPPL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn lt_rs_pdf(
    app: *mut c_void,
    path: *const c_char,
    len: usize,
    media_width: u32,
    media_length: u32,
    no_scaling: c_int,
) -> c_int {
    boundary(1, || {
        if app.is_null()
            || path.is_null()
            || len == 0
            || len > 4096
            || media_width != geometry::MEDIA_WIDTH
            || !(geometry::MEDIA_MIN_LENGTH..=geometry::MEDIA_MAX_LENGTH).contains(&media_length)
            || no_scaling == 0
        {
            return 1;
        }
        let bytes = unsafe { std::slice::from_raw_parts(path.cast::<u8>(), len) };
        let Ok(raster) = document::pdf(Path::new(OsStr::from_bytes(bytes))) else {
            return 1;
        };
        let requested = geometry::media_length_to_dots(media_length);
        if raster.width().abs_diff(requested) > 1 {
            return 1;
        }
        unsafe { &*app.cast::<App>() }.submit(&raster)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn panic_is_a_failure_not_an_unwind_into_c() {
        assert_eq!(boundary(1, || panic!("test panic")), 1);
    }
    #[test]
    fn ffi_rejects_null_without_accessing_a_document() {
        assert_eq!(
            unsafe { lt_rs_pdf(std::ptr::null_mut(), std::ptr::null(), 0, 1200, 5000, 1) },
            1
        );
    }
}
