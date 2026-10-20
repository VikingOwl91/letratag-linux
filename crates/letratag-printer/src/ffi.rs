use crate::{document, engine::Engine};
use letratag_protocol::{Profile, Raster};
use letratag_transport::MockTransport;
use std::ffi::{CString, OsStr, c_char, c_int, c_void};
use std::os::unix::ffi::OsStrExt;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::Path;
use std::sync::Mutex;

pub struct App {
    engine: Mutex<Engine>,
    runtime: tokio::runtime::Runtime,
}

impl App {
    pub fn new(engine: Engine) -> std::io::Result<Self> {
        Ok(Self {
            engine: Mutex::new(engine),
            runtime: tokio::runtime::Builder::new_multi_thread()
                .worker_threads(1)
                .enable_all()
                .build()?,
        })
    }
    fn submit(&self, raster: &Raster) -> c_int {
        let Ok(mut engine) = self.engine.lock() else {
            return 1;
        };
        // System integration deliberately stays mock-only before hardware acceptance.
        i32::from(
            self.runtime
                .block_on(engine.print(
                    raster,
                    Profile::Modern,
                    &mut MockTransport::default(),
                    false,
                ))
                .is_err(),
        )
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
    let model = CString::new("DYMO LetraTag 200B MOCK - uncalibrated 200-dpi layout").unwrap();
    let config = Config {
        dpi: 200,
        media_width: 1200,
        min_length: 1000,
        max_length: 20000,
        default_length: 5000,
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
            || media_width != 1200
            || !(1000..=20000).contains(&media_length)
            || no_scaling == 0
        {
            return 1;
        }
        let bytes = unsafe { std::slice::from_raw_parts(path.cast::<u8>(), len) };
        let Ok(raster) = document::pdf(Path::new(OsStr::from_bytes(bytes))) else {
            return 1;
        };
        let requested = (f64::from(media_length) * 200.0 / 2540.0).round() as usize;
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
