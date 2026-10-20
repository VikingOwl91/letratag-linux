# Hardware attempt 1, 2026-10-20: no physical output

First authorized physical print attempt against the LetraTag 200B
(`90:E5:B1:49:DA:AE`). The job was transmitted; no label was produced.
Operator confirmed: no tape output, no printed label. Motor noise was heard
during the run. This file records the evidence and the root-cause analysis.

## Command and journal

```sh
target/debug/letratag-printer hardware-send test-output/hello-linux.png \
  --address '90:E5:B1:49:DA:AE' \
  --state "$PWD/test-output/hardware-first" --columns 400 \
  --acknowledge-unverified-hardware
```

* Content: `HELLO LINUX`, 1000x80 PNG rendered to a 400x32 raster (content in
  wire rows 8..23), modern profile, body 1628 bytes.
* Writes on hardware: usable value size 20 (calibration cap), body chunks of
  17 bytes, 96 body chunks plus the 9-byte header = 97 writes, all through
  ATT Write Request (the TX characteristic offers only the BlueZ `write` flag;
  see [gatt-inventory-2026-10-20.md](gatt-inventory-2026-10-20.md)).
* Wall time: 11 seconds (18:15:19Z to 18:15:30Z). The engine reached the
  status phase, so at least one status notification arrived roughly 3-5
  seconds after the writes completed.
* CLI result: `print outcome uncertain; operator inspection required, no
  automatic retry` (the designed hardware outcome; the engine never claims
  completion on unverified hardware).
* Journal: `test-output/hardware-first/1792520119907487458.state` =
  `inflight`. No capture files (hardware-send does not capture). The journal
  blocks further jobs until the operator decision is recorded and the entry
  is archived manually (see docs/security.md).
* Device after the run: disconnected cleanly, still trusted, not paired.

## Environment at the time

* The host Bluetooth adapter power-cycles every 60 seconds (bluetoothd
  journal: Battery Provider Manager destroyed/created at :13/:16 each minute;
  PipeWire endpoints churn). The hci index changes on each cycle
  (observed hci6, hci2, hci0). The USB dongle (Realtek 0bda:a729) has
  USB autosuspend enabled with a 2 second delay. blueman-applet and
  blueman-tray are running. Two earlier connection attempts failed with
  `SafeTransport` (no transmission); one failure coincides exactly with an
  adapter power cycle in the journal (20:08:03 local). The Rust transport
  uses BlueZ's default adapter and is robust to the index changes; the
  60-second cycle remains a risk window for any transfer.
* Attempt 1 itself ran in a stable adapter window (no cycle during the run).

## DYMO SDK research, 2026-10-20 (operator-provided links)

Reviewed: DYMO product page, DYMO SDK support page, DYMO support page, and
the linked GitHub repositories (`dymosoftware/dymo-connect-framework`,
`dymosoftware/Drivers`, `dymosoftware/Beta-Version`), plus the DYMO Connect
SDK (NuGet `DYMO.Connect.SDK`) and its Python wrapper.

Conclusion: **no official DYMO SDK implements the LetraTag 200B BLE
protocol.** The SDKs target LabelWriter/LabelManager desktop printers
(USB/desktop software path); the LetraTag 200B is served by the mobile app
(DYMO LetraTag Connect) only. The BLE protocol references remain the
open-source implementations S1 (ysfchn/dymo-bluetooth) and S2
(thermal-label/letratag). No code or text was copied from any source.

Additional manufacturer facts from the reviewed pages:

* Product page states 160 DPI print quality. This conflicts with the 200 dpi
  layout value used by S1/S2 and this project. Unresolved; geometry
  calibration item, not a print blocker.
* User guide: manual cutter button (cut after printing), automatic power-off
  after a few minutes of inactivity, 4x AA alkaline batteries (6 V DC), and
  the troubleshooting table: "Label maker error: ensure the label cassette is
  inserted properly and not jammed or empty".

## Root-cause analysis

Two hypotheses, in order of likelihood. Both match the observed evidence
(motor noise, status notification received, no label).

### H1 (primary): disconnect during physical printing

S1 `bluetooth.py` (pinned commit, reviewed 2026-10-20) documents a
**two-message status sequence**: the printer first sends a PRINTING progress
notification (code 0 or 1), which S1 discards, and then sends the actual
result notification after physical printing completes:

```
# Printer sends two messages, first is the PRINTING, and second one is the
# printing result. So we discard the first message.
```

The engine's hardware path treated the first non-1 notification as terminal
and disconnected about one second later. A 400-column label takes roughly
7-9 seconds of physical printing (50.8-63.5 mm at the source-claimed 7 mm/s
print speed). Disconnecting on the progress notification aborted the print
mid-way. S2's model (single post-print notification, 5 s best-effort read,
`packages/web/src/printer.ts`) is consistent with a result notification that
arrives only after printing finishes.

Fix applied (engine, hardware path only): the first status notification with
code 0 or 1 is treated as a progress ack and never ends the status phase;
the disconnect happens only after the status phase ends (result received or
timeout). The outcome still reports uncertain and the journal stays inflight;
the terminal sequence remains uncalibrated. Mock behavior is unchanged.
Regression tests added: the first progress ack is discarded (codes [0, 2]
and [1, 0] read twice), an immediate error is not masked (code [2] reads
once), and mock completion still uses the first status.

### H2 (must be ruled out by the operator): no cassette installed

S1 `printer.py` Result enum documents code 7 (cassette missing) as never sent
in practice: "printer never sends this status, instead it will spin its gear
even cassette is not inserted, and will send a SUCCESS status instead". S2's
device file repeats: lid must be closed and batteries adequately charged
before the printer registers a print. Gear noise plus a success-looking
status plus no tape is exactly the documented no-cassette behavior.

Operator checks required before the next attempt: cassette inserted and
seated, lid closed, fresh batteries, and whether any tape (even blank)
emerged from the exit slot.

### Ruled out

* Packet format: byte-for-byte identical to S2 `protocol.ts`
  (`buildPrintPayload`/`chunkPayload`, pinned commit, reviewed 2026-10-20):
  START `1B 73 9A 02 00 00`, copies `1B 23 01`, print data
  `1B 44 81 02 <width u32LE> <32 u32LE> <raster>`, cut `1B 70 30`, status
  `1B 41`, end `1B 51`; 9-byte header with checksum; chunk index skip at 27;
  `12 34` trailer on the final chunk. The offline mock capture was decoded
  and validated against this structure.
* Chunk size: 17 body bytes per write is far below both the BlueZ
  characteristic MTU (500) and S2's bench-confirmed link MTU limit (247).
* The 1/5 status alternation for tiny labels (S2, bench-confirmed) does not
  apply: the test label is 400 feed columns wide.
* Adapter power cycle during the run: none (journal shows the cycle before
  and after the run, not during).

## Not yet known

* The actual status code(s) the printer sent in attempt 1. The engine did not
  log them; the RX cached value could not be read back afterwards because
  service resolution failed during repeated adapter power cycles. Attempt 2
  runs with the new content-free diagnostic mode, which logs every raw
  status notification (3-byte frames only) and write timing.
* Whether a cassette was installed during attempt 1 (operator to confirm).
* Terminal status semantics (which notification is the result) remain
  uncalibrated; the hardware path still reports uncertain by design.
