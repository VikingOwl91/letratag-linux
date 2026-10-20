# ADR 0001: PAPPL with a small C ABI and a Rust device core

## Approved PDF-first safety amendment

The user approved a separately maintained, opt-in PAPPL 1.4.9 document-policy
patch after review found that its streaming PWG reader cannot distinguish clean
EOF from a truncated next-page header. The MVP advertises and accepts **PDF
only**, completely spools bounded requests, then validates them in Rust before
any transport submission. PWG/URF support is explicitly withheld.

The generic patch neither parses nor splits PDF pages. Valid multipage PDFs
are preserved byte-for-byte by the framework; this driver's one-page MVP
rejects them as a whole. A separate generic sink checks that preservation.
No custom IPP server or device logic is added to PAPPL. See `patches/README.md`
for the pinned revision, reproduction steps and removal conditions.

The C ABI now consists of one server entry point and one Rust PDF callback.
The Rust page accumulator remains unit-tested but is not an enabled IPP input
path. PAPPL's internal shutdown races remain a framework risk; the executable
keeps its borrowed state alive until process exit rather than assuming a
public worker-drain guarantee. The earlier general raster discussion below
describes the evaluated architecture, not enabled raster input support.

Status: approved by the requester in the working discussion, 2026-10-18.
Addendum: ownership, thread safety, and panic boundaries are part of the contract.

## Decision

The transport uses BlueR (`bluer` 0.17.4, BSD-2-Clause) for typed BlueZ D-Bus
GATT discovery, notifications and AcquireWrite streams. This keeps controller
ownership and permissions with BlueZ instead of implementing HCI or a second
Bluetooth stack. Direct hand-written D-Bus bindings would duplicate service,
characteristic and stream handling. The library API and usable-MTU behavior
were checked against its actual source; physical compatibility remains open.

Option A. PAPPL handles IPP, job submission, queuing, generic raster/image
processing, and IPP attributes. A small C adapter translates PAPPL structures
into primitive ABI types. Rust owns the protocol, raster geometry, BlueZ
transport, PDF conversion invocation, print journal, and retry rules.
No custom HTTP/IPP server and no Python wrapper.

The system print path remains an explicitly named mock destination until
Phase 4. An experimental BLE transmission path is separate from system printing.
This prevents a normal print dialog action from accidentally printing on
physical tape.

## Comparison

| Criterion | A: PAPPL with Rust | B: CUPS 2.x filter/backend/PPD |
|---|---|---|
| Effort | Native library and bounded C ABI; job model already available | Maintain filters, backend, discovery, and model description ourselves |
| Reliability | Established IPP framework; test our own FFI lifetimes | Established scheduler; carefully control return codes and backend retries |
| Compatibility | IPP clients, CUPS 2.x IPP queue, PWG Raster; add a PDF filter | Existing CUPS 2.x installations directly usable |
| Future | Driverless integration without our own PPD | Tied to the traditional PPD/filter path |
| Labels | PAPPL supports custom_min/custom_max and media-col | PPD CustomPageSize, application-dependent dialogs |
| Maintenance | PAPPL and libcups as native dependencies; independent Rust device core | Larger CUPS-specific model/installation surface |

Both options require the same hardware calibration and conservative handling
of uncertain print outcomes. No reliability advantage has been measured.

## Alternatives reviewed

* `ipp.rs` is a Rust IPP codec with a synchronous/asynchronous client, not a
  replacement for PAPPL's complete Printer Application queue. A server built
  on it would require additional job, attribute, cancellation, and spool semantics.
  Reviewed: README and repository structure on 2026-10-18,
  <https://github.com/ancwrd1/ipp.rs>. Not added as a dependency.
* No custom Rust IPP server will be built. This does not claim an exhaustive
  assessment of all Rust IPP projects.
* Complete Rust bindings for all PAPPL structures would enlarge the ABI surface.
  The C compiler reads the actual PAPPL headers; Rust sees neither
  `pappl_pr_driver_data_t` nor CUPS headers.

## Local feasibility and limitations

PAPPL 1.4.9, commit `ae1889822d872eddd572a43ad0a4a3674fa5a9fa`, was built
locally within the project using the existing libcups 2.4.20 and native
libraries. The configure options initially used, `--disable-usb` and
`--disable-pam`, were not recognized by upstream. The build instructions use
the actual available options, `--disable-libusb` and `--disable-libpam`.

Sources at the same commit:
* `pappl/system-accessors.c:37-150`: numeric loopback listeners;
  `NULL` would mean wildcard and must not be used here.
* `pappl/printer.h:204-374`: raster callbacks, job options, and media model.
* `pappl/printer-driver.c:657-732`: custom media ranges as IPP collections.
* `pappl/job-process.c:714-851`: row callback return values are not checked
  consistently there. Rust records errors and transmits only after the entire
  job is finished, not at the end of a page.

PDF is not a built-in PAPPL decoder. Our MIME filter invokes a resource-limited
Poppler subprocess from Rust. Alternatively, system CUPS may convert PDF to
PWG Raster itself. Both paths require their own tests.

DNS-SD is disabled by default. A LAN announcement for a loopback-only service
would be misleading. Future local CUPS registration deliberately uses an IPP
URI; automatic BLE discovery provides candidates only, not automatically
installed printers.

## Consequences and open gates

* Native PAPPL dependency; building the core without the feature remains possible.
* No support for raw packets from arbitrary IPP clients.
* Only one page and one copy per job. Multiple jobs are serialized.
* 200 dpi, 30 active rows, and 12 × 10..200 mm are an uncalibrated development
  profile, not confirmed hardware capabilities.
* Physical status completion, feed, and GATT write size will be established in
  Phase 4. Until then, the experimental BLE path deliberately reports no certain
  print success and locks the journal after a transmission attempt.
