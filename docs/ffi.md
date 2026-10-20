# FFI contract

Public boundary: `crates/letratag-printer/native/adapter.h` and `src/ffi.rs`.
No PAPPL/CUPS structure is reproduced in Rust. C contains no Bluetooth logic,
DYMO commands, or bit packing. Media/resolution values come from Rust's
160 dpi layout profile (measured on the device 2026-10-20); C only exposes
them through PAPPL. The transport kind (mock, or BlueZ with an explicit
address, chunk mode and validated capability) is chosen in Rust before
`lt_serve` starts; C is transport-agnostic.

## Ownership and lifetimes

There are two ABI functions: Rust calls `lt_serve`; C calls `lt_rs_pdf`.
Direct raster callbacks reject inputs and never create Rust page handles.
`lt_serve` owns the server process. It returns only on startup errors before
worker creation. After `papplSystemRun`, it calls `_Exit(0)` without freeing
the framework or returning into Rust ownership cleanup.

| Value | Owner | C/Rust rule |
|---|---|---|
| `App` | Rust, Box in the `serve` call | C borrows the pointer until `lt_serve` returns; never calls `free` |
| Config, model string, spool path | Rust stack/CString | Remain valid during blocking `lt_serve`; C does not retain them afterward |
| `bridge` | C stack in `lt_serve` | Borrowed in the driver extension; remains live until process exit |
| PDF path | PAPPL | Borrowed only for the callback; Rust creates a private input copy for Poppler |
| Protocol packets/preview/journal | Rust | Do not leave Rust through the ABI |

No Rust-ABI structure or reference, `Vec`, `String`, `Result`, or trait object
crosses the boundary. Primitive fields have fixed integer widths; buffer lengths
use `size_t`/`usize`. C and Rust are built together for the same target. This ABI
cannot validate foreign/unknown pointers: their validity is an internal contract,
not an untrusted public plugin interface.

## Threading

PAPPL may run different jobs/clients on multiple threads. `App` uses a mutex
around the engine and journal. This serializes transmissions even when the
framework scheduler starts multiple processing threads. `App` owns the Tokio
runtime; PAPPL callbacks run outside its executor.

No page pointer crosses the ABI. Each PDF callback borrows its path and App;
document buffers, conversion subprocesses and rasters belong to Rust. No
`unsafe impl Send/Sync` replaces these rules.

PAPPL 1.4.9 uses detached clients and has no demonstrated public worker-drain
guarantee. Process-lifetime borrowing avoids our post-return use-after-free.
It does not fix PAPPL's internal shutdown races or provide graceful reusable
library shutdown. This is an executable entry point, not an embeddable server
library. Journal writes are explicitly synchronized before transfer; `_Exit`
does not run Rust destructors or flush arbitrary buffered state.

## Errors and panics

* The only Rust entry point called from C uses `catch_unwind` and returns a
  nonzero integer on errors or panics. No Rust allocation is freed in C.
* `panic=unwind` is also set in the release profile. No panic may unwind through
  C frames. A poisoned mutex is treated as an error.
* The CLI installs a content-free panic hook. Job titles, payloads, PDF metadata,
  and Bluetooth addresses are not copied into error logs.
* Allocator aborts, SIGKILL, C undefined behavior, and native library crashes
  cannot be caught by `catch_unwind`. The journal therefore treats process
  termination during a transfer as an uncertain print outcome.
* PAPPL receives only a generic error message. Details remain content-free in
  the private state journal. An error results in aborted, not a retry signal.
* PAPPL's ambiguous streaming raster completion is bypassed by the opt-in
  PDF-only document policy. Raster/URF formats are not advertised or accepted.

The patch preserves all bytes of valid multipage PDFs. This driver's MVP
rejects them as a whole during Rust preflight, before transport construction.
It never silently prints the first page of a multipage job.

## Test coverage limits

Tests cover NULL arguments, the separate Rust page accumulator, invalid inputs,
and an injected panic within the shared catch boundary. This is neither a formal
ABI proof nor a substitute for sanitizer/Miri tests. Full fault-injection
testing of every native cancellation path remains further hardening work.
