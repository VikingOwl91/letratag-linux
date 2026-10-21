# Reliability, security and CI audit, 2026-10-21

Focused audit of the implementation after the v0.3.0 release. Software
validation, hardware evidence and unverified risks are kept apart. No
physical printer was contacted, no label was printed, no release, tag or
registry package was created, and no historical evidence or calibration
measurement was rewritten.

Invariants checked before and after the changes:

* The mock transport is the default; real hardware requires an explicit
  transport selection.
* Hardware operations are never performed by tests or CI.
* A successful BLE write is never reported as a printed label.
* An uncertain outcome never triggers an automatic retransmission.
* Journal and recovery behaviour stays conservative.
* Direct IPP stays the recommended path; CUPS stays Experimental.
* Only the DYMO LetraTag 200B has been physically validated.

## Findings

| ID | Severity | Location | Status | Evidence | Impact | Resolution |
|---|---|---|---|---|---|---|
| R1 | Low | `crates/letratag-printer/src/document.rs`, `qpdf --show-npages` | Verified | Five mock PDF jobs left five `qpdf <defunct>` children of the server: the child was spawned, its first 32 stdout bytes were read, and the `Child` was dropped without `wait()`. | One unreaped child per printed label; a long-running server accumulates zombies and can exhaust the process table. | Fixed: the helper is reaped after the bounded read. Regression test `pdf_conversion_leaves_no_zombie_children`, which fails without the fix. |
| R2 | Low | `crates/letratag-printer/src/engine.rs`, journal creation | Verified by code reasoning | The queue's in-process `blocked` flag was set after the durable `inflight` marker and after the directory sync. A failure in between returned an error while the journal already held an entry, so the process kept accepting jobs although a restart would have blocked the queue. | The process and a restarted process disagreed about whether the queue was blocked. No duplicate label was possible: nothing had been transmitted at that point. | Fixed: the flag is set as soon as the journal entry exists, so both agree. Strictly more conservative; no recovery semantics were relaxed. |
| R3 | Low | `crates/letratag-printer/src/document.rs`, helper start-up | Verified by code reasoning | A missing `/usr/bin/prlimit` (or a missing `pdfinfo`, `qpdf` or `pdftoppm`) surfaced as `Error::Io("document I/O failed")`, which points the reader at the document instead of at the missing system tool. | A system without util-linux's `prlimit` fails every PDF job with a misleading message. | Fixed: helper start-up failures are reported as a distinct helper error naming the program and the `prlimit` requirement. Regression test `a_missing_helper_is_reported_as_a_helper_problem`. |
| R4 | Informational | `docs/security.md`, PDF subprocess limits | Verified | The limit table claimed a 15-second wall time for PDF subprocesses. `qpdf --check` and `pdftoppm` enforce it; `pdfinfo` and `qpdf --show-npages` are bounded by the 10-CPU-second `prlimit` cap only, because their output is collected with `wait_with_output()`. | The documented limit was stronger than the enforced one. A helper that blocked without consuming CPU would not have been killed by the wall clock; a CPU-bound helper is still capped. | Documentation corrected to state the enforced limit per step. No timeout semantics were changed. |
| R5 | Informational | `crates/letratag-printer/src/document.rs`, process execution | Existing limitation, no change | `prlimit` and the Poppler and qpdf tools are started by absolute path, so no environment lookup can substitute a binary. The helpers inherit the server's environment and working directory, and the IPP client cannot influence either. | The helpers are only as trustworthy as the account that starts the server. | Kept as a deliberate constraint. The absolute paths stay; only the error reporting changed. |
| R6 | Informational | `crates/letratag-printer/src/engine.rs`, journal states | False positive | The engine reads the journal value `failed-safe`, which no code path writes. | None: a failure before transmission creates no journal entry at all, so the value is never needed. | No change. The value stays accepted for compatibility. |
| R7 | Informational | transports, `crates/letratag-transport/src/bluez.rs` and `btleplug_backend.rs` | False positive | Both real backends set their `failed` flag before awaiting the write and clear it only after a successful write, so a cancelled or timed-out write cannot be replayed. The engine stops at the first write error and never retries. | None found. | No change. The no-retransmission invariant holds in the transport and in the engine. |
| R8 | Informational | `crates/letratag-printer/src/engine.rs`, hardware completion | False positive | On hardware the first success-like notification is treated as a progress acknowledgement and the outcome stays `Uncertain`; a second success does not confirm either. The engine only returns `Ok` for the mock transport. | None found: no hardware job can report a completed print. | No change. |
| R9 | Informational | `.github/workflows/ci.yml` | Verified | The recommended mock-only build was not exercised by CI, and the declared minimum Rust version (1.98) was not distinguished from the pinned toolchain (1.98.1). | A regression that broke the mock-only build, or that used an API newer than the declared minimum, would not have been caught. | Fixed: CI now builds the mock-only binary and asserts that its dependency graph contains no BLE backend, and it checks the workspace with the declared minimum toolchain 1.98.0. Both were verified locally before being added. |

## Changes

* `crates/letratag-printer/src/document.rs`: reap the page-count helper, and
  report helper start-up failures as a helper error.
* `crates/letratag-printer/src/engine.rs`: set the in-process queue guard
  when the journal entry is created.
* `docs/security.md`: state the enforced PDF subprocess limits per step and
  the absolute-path process policy.
* `.github/workflows/ci.yml`: mock-only build check and minimum-Rust-version
  check.
* Two regression tests in the document module.

No protocol, wire format, calibration, transport or public C ABI change was
made. The historical `HELLO LINUX` and RAW diagnostic fixtures are
unchanged.

## Addendum: helper wall-clock deadlines, 2026-10-21

Finding R4 above recorded that `pdfinfo` and `qpdf --show-npages` were
bounded by the CPU limit only, because their output was collected with
`wait_with_output()` and a capped read that had no deadline. That gap is
closed.

* All four helper invocations now run through one execution primitive with
  a 15-second wall-clock deadline that covers start-up, output collection
  and termination. The output is collected on a reader thread with a hard
  64 KiB bound, so a helper that writes more than the bound cannot fill a
  pipe and block the job.
* A helper that overruns the deadline is killed and reaped, the job fails
  with a distinct timeout error, and no partial output is used. A helper
  that cannot be terminated is reported as such instead of claiming a clean
  exit.
* The deadline was verified with a controlled helper that outlives a short
  injected deadline, with a helper that writes partial output and then
  blocks, and with repeated runs that must not accumulate zombies. All run
  without a printer.

No protocol, wire format, calibration, transport, journal or BLE change was
made, and no print job or hardware operation is involved.
