# Security and operational limits

## PDF-first preflight policy

The separately maintained opt-in PAPPL patch advertises one authoritative
accepted MIME type, `application/pdf`, and enforces the receive-time byte
ceiling before writing each block. Incomplete HTTP bodies are rejected before
submitting a file. Explicit or autodetected PWG/URF inputs are rejected before
the streaming path. Direct raster support is not enabled.

Rust then checks the regular-file size, PDF header and terminal EOF marker,
runs resource-limited `qpdf --check` (warnings also reject), requires a qpdf
page count of exactly one, and only then renders with resource-limited Poppler.
The complete rendered document must
contain exactly one supported page, with matching media length and no dark
pixels outside the provisional head area. No transport is constructed until
all preflight checks pass. This is a defined conservative acceptance policy,
not a formal PDF correctness proof or a parser-vulnerability guarantee.

Unsupported media/options are always rejected, even if a client allows
substitution. Page selection, additional orientation transforms and scaling
are rejected, not silently ignored. Multipage PDF data is preserved by the generic
framework policy but rejected before any output by the one-page product MVP.
Up to 16 completed-job metadata records are normally retained; a patched hard
admission ceiling bounds active plus recent completed records to 32 for the
configured 16/16 limits. Rejection floods encounter backpressure until the
normal cleanup interval reclaims old metadata. Completed documents are not preserved.

## Local service

The adapter explicitly binds to `127.0.0.1`, not `NULL`, `*`, or a machine-wide
hostname. No raw socket, USB gadget, web interface, or mDNS advertisement. IPP
on this loopback endpoint is unencrypted and does not authenticate individual
users. Other local users can access it. A multiuser server requires additional
authorization or an isolated local print architecture; loopback alone is not
a user boundary.

The service runs without root, additional capabilities, or Bluetooth controller
access. BlueZ is accessed through the normal system D-Bus. Whether the future
service user is authorized must be checked against the actual D-Bus/Polkit
rules. No blanket policy relaxations or controller group assignments are
installed. The default service configuration is mock-only; `--transport bluez`
is an explicit per-invocation operator choice, never an implicit fallback.

## Limits

| Resource | Limit |
|---|---|
| Input file / spool file | 8 MiB, regular files; CLI sets RLIMIT_FSIZE |
| PNG source | 8192 per axis, at most 16 million pixels; decoder allocations bounded |
| Raster core | 8000 × 32, binary pixels, checked lengths |
| Rust page-buffer unit-test API (not IPP) | 30..128 cross-tape pixels × at most 8000 feed rows, 8 bit |
| Protocol writes | At most 255 body chunks plus header, never beyond usable value length |
| IPP | 8 clients, at most 16 active jobs; hard admission ceiling of 32 active + completed jobs |
| PDF subprocess | 512 MiB address space, 10 CPU seconds, 15 seconds wall time, 8 MiB per file, 64 FDs |
| Journal | At most 4096 directory entries; no automatic deletion of entries with uncertain outcomes |

PAPPL/libcups parse HTTP and IPP before the document callback. The receive-time
document ceiling is no guarantee against bugs in those native libraries. Deliberately endless
HTTP requests are subject to framework timeouts and service limits, not a
custom Rust HTTP layer. Hostile multiuser environments require further
upload/spool quotas and parser fuzzing.

PAPPL's MaxImageSize does not protect the entire process RSS. The systemd
template adds MemoryMax, TasksMax, CPUQuota, and LimitFSIZE. The template has
not been live-tested and requires separate acceptance before production use.

PDF processing uses no shell command string. Arguments and the private file
path are passed separately to `prlimit`, `qpdf` and `pdftoppm`. These programs can still read files
belonging to its user; resource limits are not a complete sandbox. The hardened
service further restricts this visibility. Full document sandboxing/fuzzing
verification remains open.

## Uncertain outcomes, restart, and privacy

The side-effect journal is durable; the IPP queue itself is not restored or
automatically replayed after process restart. Accepted but unsent jobs can
therefore be lost on a crash. Durable held-job recovery needs a separate
production design and tests, without automatically resending uncertain jobs.

Before the first possible write, `inflight` is written to a new private state
file and synced along with its directory. Any subsequent error leaves the
engine blocked. The first write may already have been partially accepted when
the host reports an error. Therefore, no retry after a timeout, status error,
disconnect, panic, or process termination.

A file lock prevents two engine processes from using the same journal. A mutex
also serializes all jobs within the IPP process. Separate journal paths do
**not** protect against two deliberately started hardware programs accessing
the same device. Allow only one owner before the first real test.

The journal is not a general idempotency service for separately submitted IPP
requests. Before enabling a BLE-backed system queue, fault-inject lost IPP
acknowledgments, scheduler resubmissions and cancellation during preflight.
Those flows need explicit acceptance and operator gating; do not infer
end-to-end exactly-once physical printing from the mock transport tests.
Matching a device address and GATT UUIDs is also not proof of cryptographic
device identity. BLE pairing/encryption behavior remains unverified.

After confirmed mock success, the entry is `completed`. In the experimental
hardware path, even status code 0 currently results in `inflight` and an error:
the terminal sequence is not yet calibrated. There is no automatic recovery
switch. An operator must inspect the device, tape, and capture and make an
explicit, documented decision about the unresolved job. Only then may a journal
be archived offline or cleaned up manually. Do not recover by deleting state
files without explanation.

By default, logs and the journal contain no print contents or job titles.
When explicitly requested, `--capture` writes PNG previews and BLE packets with
**complete print contents** to the private journal directory. Use synthetic
test data only; do not commit or share capture files.

The build includes no automatic installation, deployment, restart, or queue
registration actions. `scripts/test-ipp.sh` starts only its own temporary mock
processes and terminates only their recorded PIDs.
