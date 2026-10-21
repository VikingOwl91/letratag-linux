# ADR 0001: PAPPL with a small C ABI and a Rust device core

## Workflow amendment, 2026-10-21

Direct IPP is the officially recommended/default client workflow. Clients
submit PDF jobs to the application's existing PAPPL endpoint at
`ipp://127.0.0.1:8631/ipp/print`, without a system CUPS queue or scheduler.
See [Direct IPP](../direct-ipp.md). CUPS queue integration is
[Experimental](../cups-experimental.md), not a required future installation
step. Direct IPP support in graphical applications varies; this decision does
not promise compatibility with every print dialog.

This changes workflow guidance only. Both client paths use the same PAPPL
IPP server, C ABI, Rust rendering/packing, and transport implementation.
The default transport remains mock; hardware output still requires an explicit
operator choice. PAPPL/libcups remain native library dependencies. Using libcups
or its `lp` client does not require running the system CUPS scheduler.

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

The service defaults to an explicitly named mock destination. The experimental
BLE transport requires an explicit selection; choosing Direct IPP does not
enable it. An ordinary submission to the default service cannot print on tape.

## Historical architecture comparison (2026-10-18)

This table records the original evaluation, not the enabled input formats or
the current recommended client workflow. The endpoint now accepts PDF only;
PWG Raster/URF input remains disabled.

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
Poppler subprocess from Rust. The historical alternative of CUPS converting
PDF to PWG Raster is not an enabled input path. Experimental CUPS integration
must deliver PDF and verify post-filter geometry and forwarded media attributes.

DNS-SD is disabled by default. A LAN announcement for a loopback-only service
would be misleading. Direct IPP uses the loopback endpoint without queue
registration. Experimental CUPS registration uses that same IPP URI;
automatic BLE discovery provides candidates only, not automatically installed
printers.

## Consequences and open gates

* Native PAPPL dependency; building the core without the feature remains possible.
* No support for raw packets from arbitrary IPP clients.
* Only one page and one copy per job. Multiple jobs are serialized.
* The current software layout profile is 160 dpi, 30 active rows, and
  12 × 10..200 mm. These values are not a universal physical-pitch guarantee;
  the length bounds are software/test values, not confirmed hardware capabilities.
  Historical apparent feed values near 160 versus 316 dpi remain unresolved.
  See the [resolution investigation](../evidence/resolution-investigation-2026-10-21.md).
* Phase 4 collected status/feed observations and a per-device validated GATT
  write size (chunk sizing 2026-10-21). Those observations do not establish
  universal geometry or certain physical completion. The experimental BLE path
  reports no certain print success and locks the journal after a transmission
  attempt; that conservative policy is unchanged.
