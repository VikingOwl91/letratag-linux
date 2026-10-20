# Hardware failure-case batch, 2026-10-20

Failure-case calibration runs (fresh batteries, new device and tape) to
resolve the terminal-status ambiguity and validate recovery after an
interrupted transmission. Operator-confirmed outcomes throughout.

## Runs

### Job C (hardware-10): lid open, fresh discovery — NOT RUN

The lid was open (operator-confirmed). A passive scan found no advertising
from the printer, so the job was not sent (the runner's availability gate
skipped it; nothing was written). Finding: **with the lid open the printer
did not advertise at scan time**, so a fresh discovery-based connect is not
possible. (Job D/E later showed a direct connect to the cached address can
still succeed; see below.)

### Job D (hardware-11): fault injection, killed mid-transfer — DONE

Lid state per operator: still open. The job connected (direct connect to the
cached address works even when the printer is not advertising), wrote the
header and chunks 1-3, and the process was killed at write 4/8
(mid-transfer). No status notification was received; no label was produced.
Journal archived with the operator decision "fault-injected mid-transfer;
partial job discarded; recovery confirmed by Job F".

### Job E (hardware-12): lid open, job accepted — PHANTOM SUCCESS

Lid still open (operator-confirmed). The job connected, all 8 writes were
ATT-acknowledged, and the printer sent a single `1b 52 00` (success)
notification 7.1 s after the last write — the same timing as a successful
physical print. **No tape emerged** (operator-confirmed: no band).

Finding: **with the lid open the printer accepts the job, runs the print
cycle and reports success (`1b 52 00`) but produces no physical label.**
Status code 0 is therefore NOT a sufficient proof of a physical print on
this device. This directly justifies the conservative completion policy
(see below) and motivates a preflight availability check in the CUPS
integration (see hardware-acceptance.md, remaining acceptance criteria).

### Job F (hardware-13): lid closed, recovery — SUCCESS

Lid closed, fresh batteries. All 8 writes ATT-acknowledged; single
`1b 52 00` (success) 7.16 s after the last write (after the physical print
duration). Operator-confirmed: `HELLO LINUX` readable, orientation correct,
label cut. This confirms **recovery after the interrupted transmission**:
the printer discarded Job D's partial job and printed Job F cleanly.
S2's recovery note ("the firmware discards the partial body on GATT
disconnect; subsequent jobs print cleanly after a fresh connection") is
confirmed on this device.

## Findings

1. **Phantom success with the lid open.** The printer reports `1b 52 00`
   after the print cycle even when no label can be produced (lid open). A
   code-0 result alone must not be treated as proof of a physical print.
2. **Recovery after interrupted transmission confirmed.** A job sent after
   a mid-transfer kill and link drop prints cleanly; the partial job is
   discarded by the firmware.
3. **Availability is observable by advertising.** With the lid open the
   printer was not discoverable by scan at least once. A preflight
   advertising check before accepting/sending a job would reject
   "printer unavailable (off or lid open)" cleanly instead of sending a
   job into a phantom success.
4. **Completion policy decision (2026-10-20):** kept conservative. The
   hardware path still reports uncertain and keeps the inflight journal;
   the operator inspects and decides. Auto-completion on code 0 is
   rejected for now because of the phantom-success finding (code 0 is not
   sufficient). A future auto-completion policy would need an additional
   condition (e.g. preflight availability plus a print-duration wait).
5. **Status sequence, 5 consistent success observations** (attempts 7, A, B,
   E-as-phantom, F): exactly one `1b 52 00` notification, arriving after
   the physical print duration (~7.1-7.2 s after the last write for a
   400-column job). The S1 two-message model does not match this firmware's
   successful jobs. The engine's conservative handling (discard the first
   0/1 notification, end the status phase on a later notification or
   timeout, never claim completion) remains safe for all observed cases.
6. **Reconnect settle time.** After a disconnect, an immediate reconnect
   can fail (SafeTransport); a few seconds of settle time avoid it. Not a
   printer fault; the transport reports it safely.

## Optional follow-up

Job G (hardware-14): controlled lid-open run with precise operator
choreography to re-confirm the phantom-success finding cleanly (lid open,
direct connect, job sent, observe `00` after the print cycle, no tape,
lid closed). Approved by the operator.

### Job G result (hardware-14): phantom success cleanly confirmed

Lid open (operator-confirmed at Checkpoint 0), printer on. The job
connected, all 8 writes were ATT-acknowledged, and the printer sent a
single `1b 52 00` (success) 7.16 s after the last write — the same timing as
a successful physical print. **No tape emerged** (operator-confirmed at
Checkpoint 1: lid closed again, no band). The phantom-success finding is
now confirmed by a controlled run: **with the lid open the printer accepts
the job, runs the print cycle and reports success without producing a
label.**
