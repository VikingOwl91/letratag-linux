# Hardware acceptance: LetraTag 200B

## Current guidance, 2026-10-21

[Direct IPP](direct-ipp.md) is the recommended/default workflow:
PDF → direct IPP → shared PAPPL endpoint → Rust rendering/protocol →
BlueZ BLE → printer. [CUPS is Experimental](cups-experimental.md); it adds
an upstream filtering/scheduling boundary, not another hardware backend.
This documentation change authorizes no physical print, BLE operation,
queue change or service activation. The CLI still defaults to mock transport;
real hardware requires explicit `--transport bluez` and an operator-verified
address. No firmware update, controller reset or undocumented persistent
setting is permitted.

First physical output was confirmed on 2026-10-20 (attempt 7). Direct IPP
produced a readable 50 × 12-mm label on 2026-10-21, without ruler measurements.
That is readable-output acceptance, not dimensional acceptance. Existing
forensic evidence records mock-only direct IPP controls for both 50 × 12 mm
(`media-col` 1200 × 5000) and 63.5 × 12 mm (1200 × 6350), producing 315 and
400 columns respectively. The 63.5-mm direct path has no physical acceptance.

The software uses a 160-dpi layout model. Native isotropic pitch is not
established: the historical ~159/160-dpi text inference conflicts with later
conditional apparent feed values around 316 dpi. CUPS `pdftopdf` demonstrably
shrinks the final calibration artwork by 0.787402 on both axes, but that
finding does not resolve the older pitch conflict. See the authoritative
[geometry investigation](evidence/resolution-investigation-2026-10-21.md).
Historical measurements below are retained, not promoted to a resolved
calibration or a DPI fix.

### Acceptance procedure for a separately authorized future run

1. Inspect the existing printer, cassette, journals and retained artifacts
   first. Confirm adequate batteries, seated 12-mm cassette, closed lid and
   an unambiguous device identity. Do not buy replacement hardware or alter
   Bluetooth configuration to explain an unproven cause.
2. Inspect every unresolved journal and record the operator's decision before
   any new submission. Never use a fresh state directory to bypass an
   unresolved physical outcome. Use synthetic content and private journals,
   spools and captures; payloads contain the entire label.
3. Reproduce the intended direct IPP request against mock transport first,
   following [Direct IPP](direct-ipp.md). Check actual PDF bytes, explicit
   `media-col`, scaling none, orientation, one copy, final job state and
   decoded raster landmarks. A successful initial IPP response only means
   accepted/queued. No system CUPS queue is required.
4. Stop here under the present authorization. A future physical run needs
   separate operator approval of the exact document, device, profile,
   transmission and journal recovery plan. Use the already accepted legacy
   profile and only the printer's recorded validated chunk capability, not
   a capability inferred from MTU. Do not silently switch profile or transport.
5. After an authorized run, record physical readability separately from
   measured geometry. Retain raw measurements and landmark definitions,
   notification timing, write lengths, operator observations and any actual
   payload capture. Missing hardware bytes cannot be replaced with mock bytes.
6. Keep uncertain outcomes inflight until inspected. Code 0, elapsed time,
   CUPS completion and a readable label do not establish dimensional accuracy
   or safe automatic completion. The journal is not cross-request idempotency;
   client/scheduler retries or manual resubmissions need separate controls.

## Historical bring-up and acceptance records, 2026-10-20/21

The commands and proposed next attempts below record earlier work. They are
not instructions to execute now; current guidance and the forensic report
supersede their calibration conclusions and workflow recommendations.

## Before connecting

1. Record the printer model, label cassette type and observable firmware
   identifier, if one is actually available. Do not assume a Device Information
   Service or invent a firmware version.
2. Use synthetic data only. Keep packet traces private: BLE payloads contain
   the complete label contents.
3. Read `protocol.md`, especially the legacy/modern profile differences,
   `ESC p` uncertainty, packet size accounting and ambiguous status codes.
4. Review the exact command body and every operation that may affect the
   device. The modern body contains START, one-copy directive, bitmap,
   `ESC p 30`, status and END. No cassette setting, density, reset or update.
5. Stop other processes that might own this printer through their ordinary
   user interfaces. Do not kill unrelated Bluetooth/system services.
6. Use a fresh private journal. Preserve any existing unresolved journal until
   the previous outcome has been explicitly inspected and recorded.

## 1. Identify the device

Use the ordinary BlueZ tools as the intended user, not a root daemon. Scanning
and connection require deliberate operator action; none of these steps ran
during software development. Do not power-cycle or reconfigure the controller
to make a test pass.

Names beginning with `Letratag` or `DYMO LT-200B` are candidates, not identity
proof. Check the discovered GATT service and the matching characteristic tails
against `protocol.md`. Save the actual service UUID, TX/RX UUIDs and flags.
Reject ambiguous services, a mismatched tail, missing notifications, or a TX
characteristic that offers no write mode at all. The verified LT200B offers
only the BlueZ `write` flag (ATT Write Request) on TX; the transport prefers
write-without-response when a device offers it and otherwise uses an
acknowledged Write Request. It never silently substitutes another
characteristic or mode.

Operator note (2026-10-20): the battery-powered LetraTag powers itself off
after roughly five minutes idle. If the device is unavailable or a connection
is refused, ask the operator to switch it on again; do not power-cycle or
reconfigure the controller to make a test pass.

## 2. Connection and status

Connect without printing first using normal BlueZ/GATT tooling. Verify service
resolution and a notification subscription. Record whether AcquireWrite works
and the usable payload size reported by the Rust transport.

No standalone status command is implemented because its safe framing and
semantics are not established. Do not send a bare `ESC A` merely to complete
a checklist. Passive notification observation may produce no messages until
a job is submitted. Record that as unsupported/unobserved, not success.

## 3. First deliberately authorized physical print

Start with an asymmetric synthetic black-and-white PNG, about 40 feed columns
by 30 content rows. Avoid an all-black block or an extremely short label.
Render a preview and inspect its orientation and complete packet list first.

Write size: `--chunk-mode auto` uses the hardware-validated capability from
`<state>/printer-caps.conf` (244 bytes for the verified device, confirmed by
three operator-checked prints) and otherwise falls back to the conservative
20-byte limit. `--chunk-mode safe` forces 20 bytes; `--chunk-mode manual
--max-write <bytes>` is explicit and experimental above the validated cap.
The negotiated MTU never proves printer support; see
[protocol.md](protocol.md) section 4. Automatic feed padding remains
unimplemented.

The experimental command requires both the BLE feature and an explicit flag:

```sh
cargo build --locked -p letratag-printer --features ble
target/debug/letratag-printer hardware-send calibration.png \
  --address '<operator-verified-address>' \
  --state "$PWD/test-output/hardware-first" --columns 40 \
  --acknowledge-unverified-hardware
```

This command **may physically print** and must not run automatically in CI or
as part of installation. The implementation deliberately reports an uncertain
outcome and retains an inflight journal even if it sees status code 0. A
completion sequence has not yet been calibrated. An error is not permission
to submit the same label again.

Record the exact sequence of notifications, timing, host write acceptance,
head activity and actual tape output. `--diagnostics` enables a content-free
diagnostic mode on stderr: connection size, per-write progress, raw status
notification bytes and timing. It never logs print contents. Use it for every
calibration run.

Preconditions verified against S1/S2 and the user guide before each attempt:
a DYMO LT cassette inserted and seated (S1: without a cassette the printer
spins its gear and reports success while printing nothing), lid closed, and
adequate battery charge (S2: the printer does not register a print
otherwise). Confirm whether any tape, even blank, emerged from the exit slot.

### Attempt 1 record, 2026-10-20

`HELLO LINUX`, 400 feed columns, modern profile, 97 writes through ATT Write
Request. Result: **no physical output** (operator-confirmed; motor noise was
heard). The engine disconnected about one second after the first status
notification, aborting the in-progress print; see
[evidence/hardware-attempt-1-2026-10-20.md](evidence/hardware-attempt-1-2026-10-20.md).
The hardware path now discards the first 0/1 progress notification and
disconnects only after the status phase ends. The inflight journal entry was
archived after the operator decision (no physical output observed). Attempt 2
runs with `--diagnostics` and a fresh journal.

### Attempt 2 record, 2026-10-20

Same content and profile, with `--diagnostics`. All 97 writes were
ATT-acknowledged; the printer answered `1b 52 02` (print failed) 0.3 s after
the last write, before any physical printing could occur. Operator observed
motor noise, a fast-blinking LED and no tape output. The job content is
rejected at validation; the transport, framing and write mode are ruled out
by the trace. See
[evidence/hardware-attempt-2-2026-10-20.md](evidence/hardware-attempt-2-2026-10-20.md).

`hardware-send --profile <modern|legacy>` selects the wire profile
explicitly; it is never switched automatically. Attempt 3 uses
`--profile legacy` (S1 format) on a fresh journal.

### Profile selection policy

Both profiles are documented in [protocol.md](protocol.md) and golden-tested
in `letratag-protocol`. The firmware decides which one it accepts; the
operator selects explicitly per calibration run. No automatic firmware
detection or fallback transmission. Once the accepted profile is known for
this device, record it here and in the evidence file.

### Attempt 7 record, 2026-10-20: SUCCESS (first physical print)

`--profile legacy --max-write 244 --linger-seconds 20`, fresh journal
`test-output/hardware-7`. All 8 writes ATT-acknowledged. The printer
physically printed for ~7.2 s (400 feed columns) and sent a single
`1b 52 00` (success) notification **after** printing completed.

Operator confirmation: `HELLO LINUX` readable on the label, orientation
correct. Ruler measurements: leading blank 23.25 mm, text 24.3 mm,
trailing blank 19 mm (total 66.55 mm). Ink extent in the raster: columns
124..275, rows 8..23.

Historical feed-pitch inference: 152 text columns = 24.3 mm = **158.9 dpi**.
This was interpreted as matching the manufacturer's 160 DPI spec and as
rejecting the provisional 200-dpi layout. It is not a verified native pitch:
the later forensic report leaves the ~160 versus ~316 apparent feed conflict
unresolved. The historical inferred extra lead-in was ~3.4 mm beyond the
raster's leading blank columns.

Historical attribution for attempts 1-6: weak batteries (fail-fast rejection
with status 2); the operator replaced them with well-charged cells and
reported new device/tape. The profile/chunk-size/ESC-M experiments in attempts
2-6 were confounded. The forensic report does not treat that attribution as
isolated proof of every earlier failure.

The engine still reports uncertain by design (the terminal-status
sequence is not yet calibrated into the completion policy). The inflight
journal entry was archived with the operator decision "physically
completed, operator-confirmed".

### Failure-case batch, 2026-10-20

Fresh batteries, new device and tape. See
[evidence/hardware-failure-cases-2026-10-20.md](evidence/hardware-failure-cases-2026-10-20.md).

* **Job D (fault injection):** CLI killed mid-transfer (at write 4/8), link
  dropped. No label. The firmware discarded the partial job.
* **Job E (lid open):** the printer accepted the job, ran the print cycle
  and reported `1b 52 00` (success) after ~7.1 s — but **no tape emerged**
  (operator-confirmed). **Phantom success: status code 0 is not proof of a
  physical label on this device.**
* **Job F (lid closed, recovery):** printed cleanly (`1b 52 00` after
  7.16 s), operator-confirmed and cut. **Recovery after an interrupted
  transmission is confirmed** (S2's discard-on-disconnect note holds on
  this device).
* **Job C (lid open, fresh discovery):** not run; with the lid open the
  printer did not advertise at scan time, so a discovery-based connect was
  impossible. A direct connect to the cached address still worked (Job D).

**Completion policy decision (2026-10-20):** kept conservative. Code 0 is
not sufficient for auto-completion (phantom-success finding). The hardware
path still reports uncertain and keeps the inflight journal; the operator
inspects and decides. A future auto-completion policy needs an additional
condition (e.g. preflight availability check plus a print-duration wait).

### Connection info-extraction pass, 2026-10-20

Full connection reverse-engineering pass (full GATT dump, RX+AUX
notification subscriptions, state reads around ESC M and a byte-identical
print job). See
[evidence/connection-info-2026-10-20.md](evidence/connection-info-2026-10-20.md).

Extractable: device identity (DIS strings), job status (RX read + notify:
idle `1b 52 00`, PRINTING `1b 52 01` during a job, result pushed after the
print), a short-command echo/status value on AUX (`00 00 00 00` ->
`01 00 00 00` after ESC M), on/off via advertising presence.

Not extractable on this firmware: battery state, lid state, tape/cassette
presence, jam/busy flags. No Battery Service in GATT, no manufacturer-data
broadcast, no spontaneous notifications in a 15 s idle watch. The unknown
GAP characteristic 0x2aa6 reads `00` in every state. This also corrects the
DIS strings: Hardware Revision `REV-E`, Firmware Revision `RUTEP20220628`
(the first decimal decode of those two was a transcription error).

### Job G record, 2026-10-20: controlled lid-open run, phantom success confirmed

Lid open (operator-confirmed), printer on. The job connected, all 8 writes
were ATT-acknowledged, and the printer reported `1b 52 00` (success)
7.16 s after the last write — the same timing as a successful physical
print — but **no tape emerged** (operator-confirmed). The phantom-success
finding is now confirmed by a controlled run. The first connect attempt
failed while the printer was still booting (SafeTransport, nothing sent);
after a settle delay the direct connect succeeded.

### Calibration batch, 2026-10-20: repeatability and geometry

Two further byte-identical jobs (A: `hardware-8`, B: `hardware-9`), both
operator-confirmed (`HELLO LINUX` readable, orientation correct). See
[evidence/hardware-calibration-batch-2026-10-20.md](evidence/hardware-calibration-batch-2026-10-20.md).

* The status sequence is consistent across now three successful prints:
  exactly one `1b 52 00` result notification, ~7.1-7.2 s after the last
  write (after the physical print duration). No 1/5 alternation with fresh
  batteries and 400-column content.
* Historical measurements: feed 152 text columns = 24.3 mm = 158.9 dpi;
  across 16 text rows = 2.25-2.5 mm (2.54 mm expected at 160 dpi). The
  original isotropic-160 interpretation is not confirmed native calibration.
  These coarse/corrected text measurements and later conditional ~316-dpi
  feed estimates remain in conflict; retain both rather than stretch an axis.
* Completion policy decision: kept conservative. The observed contract
  (single result notification after printing) makes the current handling
  safe, but attempt 3's two-notification sequence under weak batteries
  leaves the PRINTING-ack ambiguity unresolved. The hardware path
  therefore still reports uncertain and keeps the inflight journal; the
  operator decides. Auto-completion on code 0 is a deliberate future
  decision after failure-case observations with fresh batteries.

## 4. Raster orientation and geometry

Use an asymmetric marker to distinguish rotation, mirroring, feed direction
and row order. Then test isolated wire rows 0, 1, 7, 8, 24, 30 and 31 using a
reviewed calibration program built on `letratag-protocol`, not by changing UUIDs
or inventing opcodes. The regular image renderer defaults to 30 centered rows;
it does not itself prove that both edge rows physically print.

Measure with a ruler/caliper:

* actual dark-dot pitch across the head and along feed;
* which of the 32 addressed wire rows produce marks on the installed tape;
* leading blank tape, trailing blank tape and head-to-manual-cutter offset;
* output of 100 and 200 feed columns, including any systematic X/Y ratio;
* total cut length versus the nominal PDF media length.

Compare measurements against the software's 160-dpi model, conflicting
160/200-dpi source claims and the unresolved later ~316-dpi apparent feed.
Keep the raw measurements, not just a chosen DPI value. Do not calibrate
length by stretching only one axis until text happens to look right.

## 5. Feed, completion and repeated jobs

Test wider content before investigating the foreign tiny-label 1/5 alternation
report. Do not solve a rejected job by automatically resending it. Test a
candidate trailing-feed adjustment only as an explicit, reviewed experiment,
and distinguish added feed from requested label length.

After the terminal-status contract and disconnect behavior are established,
update the Rust completion policy and tests. Only then enable a hardware-backed
system queue. Validate several separately submitted jobs in order, including
power-off, low battery, lid/cassette conditions and a disconnect during transfer.
Do not deliberately jam tape or interfere mechanically with the print head.

## 6. Historical Experimental CUPS integration checklist

1. Retain a dedicated mock queue for regression tests. **Done:** the CUPS
   path was validated against the mock transport before the physical print
   (temporary queue `letratag-e2e-mock`, since removed).
2. Register a **new** local IPP target only with operator approval. Do not
   replace existing CUPS printers. **Done 2026-10-21:** dedicated queue
   `letratag-e2e` (driverless IPP Everywhere). The earlier run record reported
   removal after acceptance; current queue state has not been inspected here.
   The later forensic investigation left system queues intact. The existing
   `Canon_LBP633C` queue was reported untouched during acceptance.
3. Print one synthetic 50 × 12-mm PDF with `print-scaling=none`, then a
   different supported length using an explicit IPP media collection.
   **Partially done 2026-10-21:** the 50 × 12-mm PDF printed through CUPS
   (see section 7); a different length through CUPS remains untested.
4. Print from a normal Linux application/graphical dialog and compare its
   pixel preview and physical measurements with the CLI output.
5. Verify the application did not introduce page fitting, margins, extra
   copies or resubmission on an uncertain outcome. **Done for CUPS:** the
   queue used `printer-error-policy=abort-job`; exactly one transmission and
   no duplicate labels were observed in that run (operator-confirmed).
   `abort-job` is not a guarantee against all scheduler/client resends.
6. Check the non-root service user's actual BlueZ/D-Bus permissions and all
   systemd restrictions, without loosening global Bluetooth policy by default.
7. Fault-inject cancellation during preflight and lost IPP acknowledgments.
   Confirm that scheduler/client resubmissions cannot silently repeat a
   possibly printed label. The existing journal is not cross-request
   idempotency, and mock completion is not a physical exactly-once guarantee.

## 7. Experimental CUPS: historical readable-output acceptance, 2026-10-21

This was a readable-output pass, not dimensional-accuracy acceptance. See
[CUPS Experimental](cups-experimental.md) before considering this path.
The setup below is historical and is not authorized for execution now.

Full path verified: `lp` → CUPS scheduler → IPP → PAPPL
(`letratag-printer`) → Rust BLE transport → BlueZ → DYMO LetraTag 200B.
Label `CUPS-E2E-0001` (50 × 12 mm, 160 dpi layout, legacy profile),
**one physical print, operator-confirmed**.

### Setup (reproducible, reversible)

```sh
# 1. Local endpoint with the real BlueZ transport (fresh private journal,
#    validated-caps entry 90:E5:B1:49:DA:AE 244 in printer-caps.conf).
target/debug/letratag-printer serve \
  --state test-output/acceptance/cups-state \
  --spool test-output/acceptance/cups-spool \
  --port 8631 --transport bluez --address 90:E5:B1:49:DA:AE \
  --profile legacy --diagnostics

# 2. Dedicated driverless queue (IPP Everywhere), abort-job policy.
lpadmin -p letratag-e2e -E -v ipp://127.0.0.1:8631/ipp/print -m everywhere
lpadmin -p letratag-e2e -o printer-error-policy=abort-job

# 3. One print job through CUPS.
lp -d letratag-e2e -o media=12x50mm.Borderless test-output/acceptance/cups-e2e-0001.pdf

# Historical cleanup command; current queue state is not asserted here:
lpadmin -x letratag-e2e
```

Notes: the driverless PPD advertises CUPS' converted input formats while
the endpoint stays PDF-only. The adapter's scaling-none default does not
prevent upstream CUPS PDF fitting. This run used `12x50mm.Borderless`
(dimensions 1200 × 5000 in 1/100 mm); other lengths need resolved media
and post-filter geometry checks against a mock. See the
[geometry investigation](evidence/resolution-investigation-2026-10-21.md).
blueman-applet must be stopped during hardware runs (it stops
BLE discovery within a second). The printer must be powered on; if BlueZ
no longer knows the device, the transport rediscovers it (bounded LE
discovery, exact address match).

### Evidence

* CUPS: job `letratag-e2e-43` submitted 03:08:10, finished 03:08:50;
  `printer-error-policy = abort-job`; queue idle with no reasons afterwards.
* Mock pre-validation: the identical CUPS path against the mock transport
  (temporary queue, since removed) completed in one attempt; the capture
  decoded to width 315 (50 mm at 160 dpi), ink inside the printable band.
* PAPPL diagnostics (`test-output/acceptance/cups-server.log`): connected
  0.77 s, effective write size 244 (auto + validated cap), 7 writes
  (header + 6 chunks; legacy body 1284 for width 315), single `1b 52 00`
  6.2 s after the last write (physical print duration), outcome uncertain
  at 37.7 s (30 s status timeout after the first notification is discarded
  as a progress ack).
* Journal: exactly one entry, `inflight`, archived with the operator
  decision (`test-output/acceptance/cups-state/archive/DECISION.md`).
* Operator confirmation (Phase 4 checklist): label produced, text
  `CUPS-E2E-0001` present and readable, orientation and positioning
  correct, not cropped or truncated, **no duplicate or unintended labels**.

### Software-level completion vs. physical success

The three software-level states diverge deliberately and must not be read as
physical proof:

| Layer | Job state | Meaning |
|---|---|---|
| PAPPL endpoint (job 1) | `aborted` | Engine outcome uncertain (phantom-success policy); journal inflight |
| CUPS scheduler (job 43) | `completed` / `job-completed-successfully` | CUPS-side processing finished; not evidence of a physical label |
| Physical label | operator-confirmed | The actual acceptance criterion |

A successful API response, CUPS job completion, or PAPPL status alone is
not sufficient evidence of a successful print.

### Failure behavior (Phase 5)

* **Printer off / unreachable:** connect fails before any write
  (`SafeTransport`); no journal entry; CUPS job ends aborted (abort-job
  policy; no retry observed in that run). Verified live on 2026-10-21
  (two connect failures before the printer was powered on; nothing was sent
  either time).
* **BLE discovery failure:** bounded LE discovery (15 s, 256 events, exact
  address match) then `DeviceNotFound` → `SafeTransport`. Covered by
  deterministic transport tests.
* **PAPPL daemon unavailable:** CUPS jobs pend or abort per queue policy;
  no partial transmission is possible (the endpoint is the only writer).
* **Unsupported format/media:** the endpoint is PDF-only and rejects
  raster/URF, multipage, oversized and media-mismatched documents before
  any submission (loopback suite, 67 IPP assertions).
* **Job failure propagation:** pre-transmission rejections abort the IPP
  job (`print-and-expect-aborted`); post-transmission uncertain outcomes
  abort the endpoint job and leave the journal inflight. CUPS may report
  its own completion regardless (see the table above).
* **Recovery after a failed job:** the inflight journal blocks the queue
  until the operator inspects and records a decision; no automatic replay.
  Engine-level disconnect, status-timeout and cancellation paths are
  covered by deterministic mock tests.

### Historical Experimental CUPS troubleshooting

For current diagnosis use [Experimental CUPS](cups-experimental.md). In
particular, authorization failures do not prove the endpoint is down, and
enabling a queue can release pending jobs. The older notes below are not
permission to perform those operations.

* `lpadmin` fails or the queue has no PPD: the endpoint is not running or
  not answering — start it first and verify with
  `ipptool ipp://127.0.0.1:8631/ipp/print`.
* Job stays pending: endpoint down, or queue disabled — check
  `lpstat -p letratag-e2e` and `cupsenable letratag-e2e`.
* Job aborts immediately ("Input rejected or print outcome uncertain"):
  document validation failed — one 12 mm page, at most 200 mm long, ink
  inside the centered 30-row printable area, qpdf-clean PDF.
* Connect fails with `SafeTransport`: inspect diagnostics and journal to
  confirm nothing was sent. A new submission requires an operator decision;
  an error alone does not authorize another attempt.
* `Device ... not available` / connect failures: BlueZ no longer knows the
  device — re-scan (`bluetoothctl --timeout 10 scan le`); the transport
  also rediscovers automatically on the next job (bounded LE discovery).
* blueman-applet running: stop it; it stops BLE discovery within a second.
* CUPS shows completed but no label emerged: check the journal (inflight =
  uncertain outcome) and inspect the tape; do not resubmit blindly — the
  journal blocks the queue until you record a decision.
* `Unable to cancel print job` in `lpstat -l`: informational; the job
  already finished.

## Geometry investigation: software shrink demonstrated (2026-10-21)

Offline forensics demonstrates that CUPS `pdftopdf` shrank the final
calibration artwork by 0.787402 on both axes into a 50-mm output page.
The hardware log supports 315 transmitted columns, not the intended 400;
actual hardware payload bytes were not retained. This experiment does not
establish variable feed stepping. The older apparent feed-pitch discrepancy
remains unresolved. See the independent evidence record and its limits:
[evidence/resolution-investigation-2026-10-21.md](evidence/resolution-investigation-2026-10-21.md).
No rendering, protocol, or transport change was made. No additional
physical printing is authorized. Verify post-filter geometry and resolved
media dimensions against a mock before considering production changes.
The submitted media keyword did not guarantee the actual selected size.
The offline scaling-none control retained 400 columns and unchanged artwork,
but it is not a complete production fix: actual scheduler-forwarded media-col
and post-filter geometry still need authorized mock-only acceptance.

## Remaining acceptance criteria (updated 2026-10-21)

Hardware bring-up produced readable output; the shared PAPPL endpoint can
submit through Rust and BLE behind explicit `--transport bluez`. Direct IPP
is the recommended workflow, not a hardware-backed CUPS system queue.
The current 160-dpi software model remains unchanged. Still open:

1. Terminal-status auto-completion policy: resolve the PRINTING-ack
   ambiguity (failure-case observations with fresh batteries, e.g. lid
   open), then decide whether a code-0 result marks the journal completed.
   Until then the hardware path stays operator-in-the-loop (uncertain
   outcome plus inflight journal).
2. Physical geometry: **unresolved**, not a completed recalibration gate.
   PDF page geometry, IPP conversion and tests derive from the software's
   160-dpi constant. Historical ~159/160-dpi text inference and later
   conditional ~316-dpi feed estimates do not establish universal native
   pitch. Recover existing payload/measurement provenance first; do not change
   DPI or stretch an axis from these records. Historical extra lead-in
   (~3.4 mm) and actual cut length are not modeled physical guarantees.
3. Direct IPP production acceptance: readable 50 × 12-mm output is recorded,
   not measured accuracy. The 63.5-mm direct control is mock-only. Check
   non-root service-user D-Bus/BlueZ permissions (see [security.md](security.md)),
   availability, cancellation and lost acknowledgments before unattended use.
   Experimental CUPS additionally needs resolved forwarded media-col and
   complete post-filter geometry checks against mock, different lengths and
   application/dialog behavior. Any system-wide queue change needs separate
   operator approval. The historical CUPS readable label does not close these.
4. Host adapter stability: historical observations include discovery stopped
   within about a second by a running blueman-applet, adapter property/index
   churn and a lost BlueZ cache. Their current state was not inspected here.
   Establish an operator-approved coexistence policy without global Bluetooth
   changes or automatic retransmission of uncertain jobs.
5. Fault injection: disconnect during transfer, power-off mid-job,
   lid/cassette conditions with fresh batteries, and scheduler
   resubmission behavior (the journal is not cross-request idempotency).
   Engine-level disconnect/status-timeout/cancellation paths are covered by
   deterministic mock tests (2026-10-21).
6. The systemd template in packaging/ is not live-tested.

## Acceptance record

Record separately: unit tests; mock transport integration; local IPP/system
integration; physical hardware test. For each physical experiment retain the
source revision, Rust/PAPPL/BlueZ versions, actual GATT profile, cassette,
packet-profile choice, status sequence, photo and measurements. A matching
mock packet stream is not evidence of successful BLE communication or print.

Physical hardware test: executed 2026-10-20. First print succeeded (attempt
7); calibration batch A/B repeated readable output and recorded measurements,
not a now-resolved native pitch. Evidence:
docs/evidence/hardware-attempt-7-2026-10-20-success.md and
docs/evidence/hardware-calibration-batch-2026-10-20.md.

End-to-end serve-path acceptance print: executed 2026-10-21, one attempt,
operator-confirmed physical label (`ASSET-0001` / `DYMO LT200B`). Evidence:
docs/evidence/hardware-acceptance-serve-2026-10-21.md.

CUPS historical readable-output acceptance print: executed 2026-10-21,
one attempt through `lp` → CUPS → PAPPL → BLE, operator-confirmed physical label
(`CUPS-E2E-0001`). Evidence: section 7 above and
test-output/acceptance/cups-state/archive/DECISION.md.

v0.3.0 release acceptance prints: executed 2026-10-21, exactly two attempts
through the Direct IPP path (normal 50 × 12-mm label; calibration label with
explicit media-col 1200 × 5000), both operator-confirmed readable with
correct orientation. The calibration label measured 20.25 × 4.75 mm against
a transmitted 252-dot × 30-row pattern expected at 40.0 × 4.76 mm in the
160-dpi layout: feed 49.4% short (~316 dpi apparent), across on target
(~160 dpi), total 43.50 mm against ~53.4 mm expected. Preflight payload
hashes, diagnostics and journal decisions are in
test-output/release-state/archive/. This is a readability PASS and a
dimensional-accuracy FAIL; see the
[release readiness report](evidence/release-readiness-v0.3.0-2026-10-21.md).

BLE lifecycle test: executed 2026-10-21 (non-printing; see
[protocol.md](protocol.md) section 2 "Connection lifecycle"). Discovery and
connection succeed without pairing; the device object is not guaranteed to
exist before discovery; a local disconnect does not immediately remove it.

Historical first serve-path submission: on 2026-10-21 it failed at connect
(`Error::Connect`, nothing sent, no journal) because the BlueZ device cache
had been lost and the printer was unreachable; the transport now resolves
absent devices via bounded discovery (commits `6b03012`, `26227fc`).
The later direct IPP readable-output acceptance is recorded above; it is not
still in progress and does not establish dimensional accuracy.
