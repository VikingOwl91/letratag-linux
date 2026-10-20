# PAPPL document receive policy

`pappl-1.4.9-document-policy.patch` targets PAPPL 1.4.9 at upstream commit
`ae1889822d872eddd572a43ad0a4a3674fa5a9fa`. It is a separately maintained,
opt-in API extension, not an upstream release or an IPP certification claim.
The derivative patch follows PAPPL's Apache-2.0 license; see the retained
`docs/legal/PAPPL-LICENSE.txt` and `PAPPL-NOTICE.txt`.

## Why this patch exists

At that revision, `make_attrs` in `pappl/printer-driver.c` always advertises
PWG and Apple raster. `_papplJobCopyDocumentData` streams those formats to
`_papplJobProcessRaster` before receiving the whole document. Its normal file
receiver has no document byte bound and submits a file after a zero-byte read
without checking whether the HTTP body is complete. These paths do not provide
the PDF-first MVP's required receive-before-validation boundary.

Completed-job history is not a hard allocation limit: cleanup retains jobs
younger than 60 seconds even when the configured history count is exceeded.
A stream of rejected jobs can therefore accumulate metadata. Driver capability
attributes are also appended rather than replaced, so simply supplying narrower
capabilities can leave contradictory duplicate attributes in IPP responses.

## Opt-in contract

The public header defines `PAPPL_DOCUMENT_POLICY_API` as `1` and declares:

```c
bool papplSystemSetDocumentPolicy(pappl_system_t *system,
                                 const char *format, size_t max_job_bytes);
```

During serialized setup, call this with `"application/pdf"` and a positive
document byte limit, before creating or loading printers and before
`papplSystemRun`. Abort application startup if it returns false. It rejects
null arguments, zero limits, formats longer than 255 bytes, and malformed or
non-lowercase MIME tokens. It copies the format and checks allocation failure.
The system write lock protects replacement; an already-running system or one
with an allocated printer array rejects changes. Do not race setup with printer
creation or system startup. There is no reset API or persistent policy setting;
configure every newly created system before loading state.

With the policy enabled:

- Existing printers also resolve by the exact CUPS-style `/printers/<name>`
  resource used by `lp`/`lpr` named-destination lookups. Canonical advertised
  `/ipp/print` URIs are unchanged. Unknown names do not fall back to the default
  printer. This is resource lookup compatibility, not a second IPP server or
  registration with the machine's CUPS scheduler.
  This lookup is used by CUPS 2.4.20 `cups/dest.c:1431-1435` and repeated by
  `cups/util.c:181` during `cupsCreateJob`, including after a default-printer
  lookup. Merely dropping `lp -d` does not remove the need for named resolution.

- Job options are treated with strict attribute fidelity even if a client asks
  for substitution. An unsupported media keyword or other unsupported option
  is rejected before job allocation, never silently replaced with defaults.
  This is important for labels: changing the requested size can produce an
  unintended label even when the submitted PDF happens to fit the default.

- Supported, preferred, and default document formats are the configured format
  only. Explicit validation and a receive-entry guard reject format mismatches before streaming.
  Explicit PWG/URF requests are rejected by the PDF policy, including when their
  bytes look like PDF. Existing octet-stream auto-detection is retained.
  Attribute validation uses PAPPL's unsupported-attribute response, with the
  rejected `document-format` in the unsupported group. The receive-entry
  defensive guard uses the document-format-specific error. Both are rejection
  responses, not permission to submit or retry a job.
- Documents are spooled completely before `_papplJobSubmitFile`, not streamed
  into the raster processing path. The limit counts document bytes returned by
  CUPS, after HTTP/IPP compression handling, not IPP attributes or wire framing.
  A read that would exceed the limit is rejected before writing that buffer.
  The partial file is closed and unlinked and the job is aborted with
  `IPP_STATUS_ERROR_REQUEST_ENTITY` (client-error-request-entity-too-large).
- Read failures, remaining HTTP body bytes, a POST still receiving data, or a
  CUPS HTTP error prevent submission and remove the partial spool file. The
  completion checks use `httpGetRemaining`, `httpGetState`, and `httpError`,
  not `httpGetLength2`, which reads the original framing length. Source inspection
  and a loopback HTTP probe against installed CUPS 2.4.20 confirmed complete and
  truncated fixed-length bodies, complete chunks, truncated chunk data, and
  missing final chunk/trailer detection. Repeat on the deployed CUPS version.
- PDF bytes are copied unchanged. This patch does not parse PDFs, count pages,
  truncate pages, or reject otherwise valid multipage documents within the bound.
- `_papplJobCreate` applies backpressure under the printer write lock, before
  allocation, when both `max_active_jobs > 0` and `max_completed_jobs >= 0`.
  It rejects creation when active plus completed jobs reach the positive sum
  of those limits. Casts precede addition and the count comparison uses
  subtraction to avoid overflowing the retained-job count. With limits 16/16,
  a printer starting empty retains at most 32 active/completed job objects,
  including aborted jobs. IPP creation uses its existing busy response. No young
  completed job is freed early; ordinary cleanup can reclaim excess history
  after the 60-second grace period, allowing creation again. Use a positive
  completed-job limit for this recovery: upstream cleanup does not remove
  history with a zero limit. Existing excess history is not eagerly reduced.
- `papplPrinterSetDriverData` treats supplied capability attributes as overrides:
  before its existing copy, it removes every matching name from both the base
  and generated driver attributes. Unnamed group separators are skipped. Supply
  one well-formed printer attribute per name in a separate input IPP message;
  duplicate names in the input itself are not deduplicated. The caller must not
  override the document format policy or create inconsistent capabilities.
  A supplied `copies-supported` also suppresses PAPPL's later generated value
  in `_papplPrinterCopyAttributesNoLock`, avoiding a second conflicting range.
  Other dynamically generated attributes are not universally overridden.
  Later driver-data updates must resupply overrides: removing a base attribute
  is not undone if a subsequent update omits its name. Replacement is not
  transactional if CUPS fails to allocate the copied attributes.

The adapter can therefore advertise `page-ranges-supported=false`,
`orientation-requested-supported=IPP_ORIENT_NONE`, `print-scaling-supported=none`,
and `copies-supported=1-1` without static duplicate values. Both attribute sets
are copied into Get-Printer-Attributes responses by
`_papplPrinterCopyAttributesNoLock`; `_papplCopyAttributes` does not replace names.
PAPPL's page-range validator reads `driver_attrs`, so the false override rejects
page ranges. The patch also checks overridden orientation, scaling and copy
values during Validate-Job and print-job preflight, before job allocation.
The adapter repeats those checks defensively. Defaults must agree with these
capabilities; this generic override does not rewrite driver defaults.
Moving an attribute out of `printer->attrs` can also affect validators that
only consult that set. The patch therefore additionally checks the configured
format directly in document validation, including Validate-Job, instead of
depending on the location of generated supported-format attributes. Other
overridden attributes still require their own validation-path checks.

Without opting in, the format list, raster streaming, job-history admission,
and attribute-copy behavior stay unchanged.
The private fields start at zero through PAPPL's existing `calloc` allocation;
the owned format is freed by `papplSystemDelete`.

This is only an IPP receive boundary, not a PDF validator or device safety gate.
The application must validate the complete spool file in Rust before any physical
action, including all pages and resource limits. PAPPL can acquire its device
before invoking an application callback; the application's device implementation
must keep acquisition side-effect-free until validation succeeds. Web uploads,
raw sockets, USB input, and direct file submission are outside this receive hook
and must stay disabled or be gated separately. There is no device-specific code,
BLE behavior, command policy, or global installation in this patch.

## Apply and verify locally

Use a separate clean source copy at the pinned revision, not the reference
checkout or a system installation:

```sh
git -C /path/to/pappl-copy apply --check /path/to/letratag-linux/patches/pappl-1.4.9-document-policy.patch
git -C /path/to/pappl-copy apply /path/to/letratag-linux/patches/pappl-1.4.9-document-policy.patch
```

Rebuild PAPPL and the consuming application together under a private prefix.
Check the feature macro at compile time and the setter result at startup.
Do not link patched headers against an unpatched PAPPL library.

Required regressions before release:

1. Setter success, invalid format/limit failures, allocation failure, rejection
   after printer creation or while running, and unchanged policy after failure.
2. PDF-only supported/preferred/default attributes and explicit PWG/URF rejection
   for Validate-Job, Print-Job, and Create-Job/Send-Document flows.
3. Exact byte-limit success and one-byte-over failure, with no submission, no
   surviving partial spool file, and no device I/O for rejected documents.
4. Premature EOF for Content-Length and chunked bodies, including missing final
   chunk, read/write failures, and clean completion with persistent connections.
5. Byte-for-byte preservation of a valid multipage PDF, full-document Rust
   validation before physical action, and rejection of invalid PDF data claiming
   the PDF MIME type. Exercise compression if enabled.
6. Unconfigured PAPPL retains its previous format advertisement and raster path.
7. A flood of aborted jobs reaches the configured active-plus-completed bound
   without another allocation; concurrency cannot exceed it. At 16/16, check 32
   retained jobs, busy responses while full, and recovery after normal cleanup.
   Include zero/disabled limits and existing excess history. Verify no eager
   deletion of jobs still borrowed by clients.
8. Get-Printer-Attributes returns exactly one of each supplied capability, both
   for all attributes and requested-attribute subsets, including copies 1-1.
   Check page-range rejection and orientation/scaling/copies rejection during
   Validate-Job with fidelity enabled and before print-job allocation. Also
   check repeated driver-data updates and unconfigured legacy behavior.

Patch applicability alone does not establish these runtime properties. No live
printer or IPP interoperability certification is implied.

## Removal conditions

Remove the patch when an upstream PAPPL API provides equivalent single-format
advertisement and bounded, complete IPP file reception before application
processing, hard retained-job admission bounds without premature deletion, and
authoritative capability overrides without duplicate generated copies ranges,
preflight enforcement of the narrowed supported values, and equivalent named
CUPS-client resource lookup without misrouting unknown names.
Migrate the caller and feature detection together, then rerun the regressions
above against that upstream version. Do not drop it merely because a newer
version compiles or still exposes a PDF callback.
