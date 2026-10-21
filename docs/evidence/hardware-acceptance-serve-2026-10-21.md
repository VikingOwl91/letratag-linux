# End-to-end hardware acceptance through the serve path, 2026-10-21

PDF → IPP/PAPPL → Rust rendering → protocol encoding → BlueZ → DYMO
LetraTag 200B. **Physical output produced and operator-confirmed.**
Exactly one print attempt reached the printer; one earlier submission failed
at connect with nothing transmitted (see "Failed first attempt" below).

## Label and asset

* Content: `ASSET-0001` / `DYMO LT200B`, two lines, 7 pt Helvetica, 50 × 12 mm
  page (`test-output/acceptance/asset-0001.pdf`, 641 bytes, qpdf-clean).
* Offline validation before the run: `preview --pdf` renders 315 columns at
  the 160 dpi layout; a mock job completes; the mock capture decodes to width
  315, body 1288 (modern profile), ink inside the centered 30-row printable
  area (feed ink 22..126 px = 16.67 mm).

## Invocation

```sh
target/debug/letratag-printer serve \
  --state test-output/acceptance/hw-state \
  --spool test-output/acceptance/hw-spool \
  --port 8631 --transport bluez --address 90:E5:B1:49:DA:AE \
  --profile legacy --diagnostics

lp -h 127.0.0.1:8631 -d letratag \
  -o media=custom_default_12x50mm -o print-scaling=none \
  test-output/acceptance/asset-0001.pdf   # request id letratag-1
```

Fresh private journal with the approved validated-caps entry
(`printer-caps.conf`: `90:E5:B1:49:DA:AE 244`). Preconditions operator-confirmed:
printer powered on, 12 mm cassette seated, lid closed, batteries charged.

## Diagnostic trace (content-free; `test-output/acceptance/server.log`)

```
serving BlueZ transport on 127.0.0.1:8631; chunk mode: Auto, validated printer cap: 244
[diag] connected; usable value size 244 at 1.807915839s
[diag] Transport:             BlueZ
[diag] Write mode:            Request
[diag] Negotiated ATT MTU:    unknown
[diag] Usable GATT value:     495
[diag] Validated printer cap: 244
[diag] Effective write size:  244
[diag] Chunk mode:            auto
[diag] write 1/7 len=9 at 1.881616554s
[diag] write 2/7 len=242 at 1.994541381s
[diag] write 3/7 len=242 at 2.107434728s
[diag] write 4/7 len=242 at 2.219446272s
[diag] write 5/7 len=242 at 2.331544129s
[diag] write 6/7 len=242 at 2.444465339s
[diag] write 7/7 len=82 at 2.519474266s
[diag] status [1b 52 00] at 8.70671754s
[diag] outcome Err(Uncertain) at 38.708495005s
```

Chunk arithmetic (legacy profile, body = 24 + 4·width): width 315 → body
1284 bytes; ceiling 241 (244 − 3 for index and final magic); 6 chunks:
5 × 242 bytes plus a final 82 bytes (1 index + 79 body + 2 magic) after the
9-byte header. Matches the trace exactly.

## Status sequence and outcome

* All 7 writes ATT-acknowledged (Write Request mode, as the verified LT200B
  TX characteristic requires).
* Single result notification `1b 52 00` (success) 6.2 s after the last write,
  i.e. after the physical print duration — the same contract observed on
  attempt 7 and calibration batch A/B (2026-10-20).
* The engine discards the first 0/1 notification as a progress ack, waits for
  a second notification that never comes, and ends the status phase on the
  30 s timeout: outcome **uncertain** by design. The IPP job is not
  auto-confirmed as physically printed.
* The service was stopped cleanly after the outcome; the journal entry
  (`1792541940475525608.state`, `inflight`) was archived with the operator
  decision "physically completed, operator-confirmed"
  (`test-output/acceptance/hw-state/archive/DECISION.md`).

## Operator confirmation

Operator inspected the physical label: printed and readable
("BAAAAM asset-0001"). Detailed ruler measurements were not taken this run;
the geometry is anchored by the offline render (315 px = 50 mm at 160 dpi,
ink 16.67 mm feed) and by the 2026-10-20 calibration batch.

## Failed first attempt (same day, nothing transmitted)

The first submission (01:49) failed at connect: `Error::Connect` at 3.4 ms.
Diagnosis: the BlueZ device cache had been lost (no `dev_90_E5_B1_49_DA_AE`
object in the D-Bus tree), a running blueman-applet stopped BLE discovery
within ~0.5 s of it starting, and the printer had auto-powered-off
(~5 min idle). No bytes were sent, no journal entry was created, and the queue
was not blocked. The transport now resolves absent devices via bounded LE
discovery and reuses existing links (commits `6b03012`, `26227fc`); the
lifecycle behavior is recorded in
[../protocol.md](../protocol.md) section 2 and was established by an
independent non-printing lifecycle test.

## Verified vs. assumed

* **Verified (hardware, this run):** the full serve path produces a physical,
  operator-confirmed label; auto chunk sizing selects the validated 244-byte
  cap; the diagnostics block reports the real transport parameters; the status
  contract matches the 2026-10-20 calibration; the journal reaches inflight
  and archives with an operator decision; the service shuts down cleanly.
* **Verified (hardware, earlier):** 160 dpi isotropic pitch, legacy profile
  acceptance, single `1b 52 00` result after printing, phantom success with
  the lid open, ~3.4 mm lead-in.
* **Not verified / assumed:** detailed physical dimensions of this label
  (ruler); terminal-status auto-completion (policy deliberately unchanged);
  long-term BlueZ device persistence; behavior across adapter power-cycles.
