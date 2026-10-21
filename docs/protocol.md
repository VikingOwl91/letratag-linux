# LetraTag 200B: protocol findings

System-input scope: the approved PDF-first IPP endpoint does not accept
PWG Raster or URF. Raster layout information below describes the Rust device
core and reference research, not enabled direct raster-stream ingestion.

[Direct IPP](direct-ipp.md) is the recommended/default client workflow;
[CUPS integration](cups-experimental.md) is Experimental. Both use the same
PAPPL server and unchanged Rust packing and BLE transport. The default
transport remains mock. Client workflow choice does not establish physical
dot pitch. The BLE backend is selectable (`bluez`, established, or
`btleplug`, alternative); both produce identical payloads and GATT write
operations (see [backend comparison](evidence/backend-comparison-2026-10-21.md)).

Research date: 2026-10-18, hardware-verified additions marked inline
(2026-10-20/21). This is not a manufacturer specification. The
implementation separates modern and legacy source profiles rather than
silently merging contradictory findings.

## Sources and evidence levels

* **Manufacturer user guide:**
  <https://download.dymo.com/dymo/user-guides/LetraTag/LT200B/LT200B_UserGuide.en.pdf>,
  retrieved 2026-10-18, SHA256
  `fbb392da33f62fdf2984f44b303410e806a3c40526ca67281d8aa7b9e7e78b6b`.
  Its printing instructions explicitly say to press the cutter button after
  printing. This supports a manual cutter, not an automatic-cut capability.
  It provides no measured dot pitch for this project. The current German
  product page returned HTTP 403, so its reported resolution specification
  was not independently revalidated during this task.

* **S1, source code:** ysfchn/dymo-bluetooth,
  [`1c1b30a6612de6bf880d5f077a23630b83810750`](https://github.com/ysfchn/dymo-bluetooth/tree/1c1b30a6612de6bf880d5f077a23630b83810750), MIT.
* **S2, source code:** thermal-label/letratag,
  [`a8fb41e19ce3a8cb1e51d6aaeb1acc8060d5842b`](https://github.com/thermal-label/letratag/tree/a8fb41e19ce3a8cb1e51d6aaeb1acc8060d5842b), MIT.
* **S3, historical comparison source:** alexhorn/lt200b,
  [`129c1f206db8a3b3f13bfae09901e12507a80c53`](https://github.com/alexhorn/lt200b/tree/129c1f206db8a3b3f13bfae09901e12507a80c53), AGPL-3.0.
  No code or text was copied from this source.
* **D, published documentation:**
  <https://thermal-label.github.io/letratag/protocol/letratag-bt>, accessed on
  2026-10-18. Partly contradicts S1 and S2.

The local source trees were unmodified; their HEADs were compared with the
current remote HEADs. This establishes the revision read, not its hardware
correctness. Reports of third-party bench tests remain **third-party observations**.

## 1. BLE services and characteristics

S1 `dymo_bluetooth/bluetooth.py:37-42` lists:

| Role | UUID used there |
|---|---|
| Service | `be3dd650-2b3d-42f1-99c1-f0f749dd0678` |
| TX | `be3dd651-2b3d-42f1-99c1-f0f749dd0678` |
| RX | `be3dd652-2b3d-42f1-99c1-f0f749dd0678` |
| Auxiliary characteristic | `be3dd653-2b3d-42f1-99c1-f0f749dd0678` |

S3 `print.py:37-45` searches for the service by its first UUID block. S2
`packages/core/data/devices/LT_200B.json5:13-25` describes deriving the
characteristics from the same service tail. Our transport requires exactly
one matching service and unique TX/RX characteristics with the same tail and
appropriate GATT flags. The full UUID above is not enforced as a global
identifier for every device.

D describes TX as write-without-response and RX as notify. In contrast, S1
`bluetooth.py:95-97` writes with `response=True`. **Hardware-verified on
2026-10-20** (see [evidence/gatt-inventory-2026-10-20.md](evidence/gatt-inventory-2026-10-20.md)):
this LT200B's TX characteristic `be3dd651` offers only the BlueZ `write` flag
(ATT Write Request, acknowledged), not `write-without-response`. The
experimental transport accepts either write mode, preferring the
low-overhead Write Command through BlueZ AcquireWrite when the device offers
it, and otherwise writes through WriteValue with an ATT Write Request. A TX
characteristic with no write flag at all is rejected. Characteristic 653
remains unused.

## 2. Connection and identification

Device names are discovery hints only. S1 `bluetooth.py:109-120` currently uses
`Letratag` and an address suffix; S2 lists `Letratag ` and historical
`DYMO LT-200B` names. D's claim that S1 generally uses only the old name is
incorrect for the commit reviewed here.

No Espressif MAC whitelist, adapter power-on, or automatic pairing. The actual
connection uses an explicitly configured address. Services are resolved through
BlueZ, and RX is subscribed before the first TX write. Successful local GATT
acceptance is not proof of printing.

### Connection lifecycle (hardware-verified 2026-10-21, non-printing)

An independent BlueZ lifecycle test on the physical printer established:

* The BlueZ device object is **not guaranteed to exist before discovery**:
  after the device cache was lost, the printer was absent from
  `bluetoothctl devices` and `info` returned `not available`.
* A fresh LE scan rediscovered it immediately; `Paired: no`, `Bonded: no`,
  `Trusted: no`.
* `connect` succeeded **without pairing**; `disconnect` succeeded with
  `org.bluez.Reason.Local`; the device object remained visible afterwards.
* The advertised name changed from `Letratag 90E5B149DAAE` to `Letratag`
  after connection, and additional standard GATT service UUIDs appeared.
* The earlier disappearance (observed during the acceptance attempt) is
  consistent with BlueZ eventually removing a temporary unpaired device;
  the exact removal timing is unproven.

Transport consequences (implemented 2026-10-21): the connect path resolves
the configured device first — if the adapter does not know the address, it
runs bounded LE discovery (15 s, 256 events, exact address match, LE-only
filter best effort) and only then connects. Discovery never pairs, bonds or
trusts. If the device is already connected, the existing link is reused
instead of failing with a second `Connect`. Connection-establishment
failures (including `DeviceNotFound`, e.g. the printer auto-powered-off
after ~5 min idle) remain distinct from failures after transmission
begins: they reject the job safely with nothing sent, while any failure
after the first write stays uncertain with no automatic replay.

### Short command: set cassette type (ESC M)

S2 `docs/protocol/letratag-bt.md` documents a stand-alone payload on the
auxiliary characteristic (`...d653`, "printShortCommandUUID"), which carries
out-of-band commands that do not go through the chunked-print pipeline:

```
HEADER[9] + ESC s + ESC M <cassetteId> 00 00 00 + ESC Q
```

One non-print write, 23 bytes, no chunking. The cassette id enum is shared
with the advertising-data cassette field: 1=6mm, 2=9mm, 3=12mm, 4=19mm,
5=24mm; the LT-200B chassis only accepts 12 mm (id 3). S2 reports the three
zero pad bytes as part of the wire format; S1 documents a 3-byte `ESC M`
without padding but never bench-tested it (S1's CLI does not use the
command), so the S2 form is implemented. S2 also reports that printing works
on every observed substrate without this command and that the vendor app
typically issues it out-of-band. It is therefore strictly opt-in
(`hardware-send --set-cassette-type`) and never sent automatically.

## 3. Initialization and status

Both sources start print jobs with `1B 73 9A 02 00 00`. The constant tail is not
a system job ID. Normal printing requires no additional initialization, reset,
density, or cassette-setting command. This is a decision based on comparing
sources, not yet a hardware finding.

`1B 41` requests status within a job. The response is `1B 52 code`.
Our parser requires exactly three bytes and preserves unknown codes unchanged.

| Code | Third-party interpretation in S1 `printer.py:45-88` | Our handling |
|---|---|---|
| 0 | Success, also possible without a cassette | Syntax: Success; hardware completion still unconfirmed |
| 1 | Comment says PRINTING or SUCCESS, later aliased to 0 | Unconfirmed(1), never sufficient proof of success on its own |
| 2 / 5 | Error | Unconfirmed, job outcome uncertain |
| 3 | Printed with low battery | Unconfirmed, not reported as confirmed |
| 4 | Aborted | Unconfirmed, no retry |
| 6 | Battery too low | Unconfirmed |
| 7 | Cassette missing, not observed according to S1 | Unconfirmed, not used as a presence sensor |

S1 `bluetooth.py` (pinned commit, re-read 2026-10-20) documents a
**two-message sequence**: the printer first sends a PRINTING progress
notification (code 0 or 1), which S1 discards, and then sends the actual
result notification after physical printing completes. S2
`packages/web/src/printer.ts:125-142` instead reads a single post-print
notification, best-effort with a 5 s timeout. D claims only code 0 has been
observed; S2's device file `:62-79` explicitly reports a 1/5 pattern. The
number and order of responses must be recorded on the specific device.

**Hardware-observed consequence (2026-10-20, attempt 1):** the engine
previously treated the first non-1 notification as terminal and disconnected
about one second later, aborting a physically in-progress print (motor ran,
no label produced; see
[evidence/hardware-attempt-1-2026-10-20.md](evidence/hardware-attempt-1-2026-10-20.md)).
The hardware path now discards the first 0/1 notification as a progress ack
and ends the status phase only on a later notification or timeout, so the
disconnect happens after physical printing can complete. The outcome still
reports uncertain and the journal stays inflight; the terminal sequence is
not yet calibrated. Mock behavior is unchanged.

**Observed on this device (attempt 7, first successful print):** the
successful job produced a single `1b 52 00` notification 7.24 s after the
last write, i.e. after the physical print duration. The S1 two-message
model (first PRINTING ack, then result) did not match the successful case.
The notification is the result and arrives after printing completes, so
disconnecting on it is safe. One observation; the completion policy is
deliberately unchanged until repeated jobs and failure cases confirm it.

**Connection-info pass 2026-10-20** (see
[evidence/connection-info-2026-10-20.md](evidence/connection-info-2026-10-20.md)):
the RX characteristic's cached value reflects the job status and is
readable at any time: `1b 52 00` idle, `1b 52 01` (PRINTING, code 1,
confirming S1's table) during a print, `1b 52 00` after a successful print.
The final result is pushed as a notification; the PRINTING state is only
read-visible. The AUX characteristic carries a 4-byte value (`00 00 00 00`
idle, `01 00 00 00` after an ESC M short command) and is the short-command
channel. No battery, lid or tape state is available: no Battery Service in
GATT, no manufacturer-data broadcast, and no spontaneous notifications in a
15 s idle watch.

D describes three-byte manufacturer advertising data including tape/battery
flags. The reviewed S2 `packages/core/src/status.ts:30-32,90` instead reports
`mediaLoaded: true` without evaluating these data. No manufacturer identifier
is invented here, and no cassette sensor is implemented.

## 4. Packet structure, lengths, and transfer

Shared by S1 `printer.py:437-482` and S2 `protocol.ts:196-213,323-381`:

```
Header = FF F0 12 34 <body_len:u32 little-endian> <sum(header[0..8]) mod 256>
Write 0 = Header, exactly 9 bytes
Write 1..N = <index:u8> <body_segment> [12 34 only in the final write]
```

The length counts body bytes only. The header, indices, and final trailer do
not count. The implementations limit each body chunk to 500 bytes. At chunk
position `i >= 27`, index `i+1` is sent, skipping index 27. We allow at most
255 body chunks so the final index remains 255.

**Important correction to D/S2:** Raw ATT MTU, usable GATT value length, and
protocol body length are different quantities. A normal ATT Write Command has
three bytes of ATT overhead. Our packets additionally need an index and, at
completion, two trailer bytes. Using `mtu-1` without this distinction is unsafe.
`bluer 0.17.4` already applies `mtu_workaround`, reducing the reported MTU by
five bytes; the transport uses the length actually accepted by its writer and
does not subtract ATT overhead twice. The encoder conservatively reserves
three bytes in **every** chunk: `min(500, effective_write_len - 3)`.

### Chunk-size selection (2026-10-21)

The effective GATT write-value size is selected at connect time:

```
effective_write_limit = min(
    transport_gatt_value_limit,   # bluer-reported usable value size
    validated_printer_limit,      # hardware-confirmed, per address
    implementation_limit,         # 503: 500 body + index + final magic
)
```

Three modes (`--chunk-mode`, `letratag-transport::ChunkMode`):

* **auto (default):** the validated printer capability from
  `<state>/printer-caps.conf`, or the conservative ATT23-safe 20-byte limit
  when no capability is recorded. The negotiated MTU alone never proves
  printer support: on the verified LT200B BlueZ reports a characteristic MTU
  of 500 (bluer usable value 495) while the validated, operator-confirmed
  printer limit is 244 bytes.
* **safe:** always the conservative 20-byte limit, or smaller if the
  transport requires it.
* **manual (`--max-write <bytes>`):** an explicitly configured size,
  validated against the negotiated transport limit and the protocol bounds
  9..=512. Values above the validated capability are reported as
  **experimental** in diagnostics; an acknowledged ATT write is not
  validation.

Validated capabilities are recorded per device address in
`<state>/printer-caps.conf` (`<address> <bytes>` per line, `#` comments).
Only operator-confirmed limits belong there; nothing is promoted
automatically, and a malformed file fails startup. Unknown values are
reported as unknown in diagnostics, never invented. On the Write Request
path (the verified LT200B TX characteristic offers only the `write` flag)
BlueZ's MTU property is a characteristic capability, not the negotiated
link MTU, so the negotiated ATT MTU is reported unknown; on the Write
Command path (AcquireWrite) it is the AcquireWrite MTU.

Oversized jobs are rejected in full before the first write. The
8000-column limit is our software limit and S1's canvas limit, not a
guaranteed device capability. A small write limit further reduces the
maximum transmittable label length.

## 5. Body profiles and raster format

**Modern**, based on S2 `protocol.ts:274-308`, the default for mock and experimental BLE:

```
1B 73 9A 02 00 00                 Start
1B 23 01                          exactly one copy
1B 44 81 02 <width:u32LE> 20 00 00 00 <4*width raster bytes>
1B 70 30                          observed completion command, semantics unresolved
1B 41                             Status
1B 51                             End
```

**Legacy**, based on S1 `printer.py:485-505`: no `ESC #`, raster marker byte `01`
instead of `81`, and `1B 45` instead of `1B 70 30`. The profile exists in the
library and reference tests. No automatic firmware selection or fallback transmission.

Raster coordinates in our core format: `x` is the tape travel/feed direction;
`y=0..31` runs across the tape. A black pixel sets a bit:

```
wire_byte = x*4 + (3 - y/8)
mask = 1 << (7 - y%8)
```

Single pixels at x=0: y=0 gives `00 00 00 80`, y=7 `00 00 00 01`,
y=24 `80 00 00 00`, y=31 `01 00 00 00`. Black is 1; white is 0.
Reference: S2 `protocol.ts:137-185`. Printer orientation is not hardware-verified.
S1 uses 30 active rows with y+1, corresponding to centering 30 content pixels
within a 32-row frame. Our renderer separates this content convention from the
fully addressable wire frame.

Literal vectors in `crates/letratag-protocol/tests/protocol.rs` were calculated
independently from these source formats; they are **not device captures**.

## 6. Resolution and printable pixels

32 is the transmitted frame height. 30 is S1's canvas convention, not a
printhead measured here. S2 lists 200 dpi; historical sources and the template
research report 160 dpi and anisotropic effects. A tape width of 12 mm is not
the print height.

The current software layout uses 160 dpi (`geometry::RESOLUTION_DPI`), not a
verified universal native pitch. The Oct 20 text measurements gave an apparent
feed value of 158.9 dpi (152 occupied columns / 24.3 mm); 16 occupied rows
across were reported as 2.25-2.5 mm. The earlier conclusion of isotropic
160 dpi was stronger than this evidence supports. Later rectangle measurements,
conditional on reconstructed post-filter pixels, imply about 311-316 dpi feed
and 164-167 dpi across. Missing hardware payloads and differing measurement
definitions leave the historical 160-versus-316 feed discrepancy unresolved.
See the authoritative
[resolution investigation](evidence/resolution-investigation-2026-10-21.md).

That investigation demonstrated a separate software shrink: CUPS `pdftopdf`
fit an unresolved 63.5-mm custom-media request to a 50-mm page, scaling both
axes by about 0.787402 before Rust rendering. Direct/mock controls preserve
the artwork; the offline scaling-none control removes that shrink. These
controls do not establish historical scheduler-forwarded IPP attributes or
resolve native pitch. No new firmware mode, anisotropic driver resolution, or
transport correction follows from them.

At the current layout resolution, 30 rows correspond nominally to 4.76 mm.
The remaining rows of the rendered 12-mm PDF page are discarded only if white.
Nonwhite content outside the centered band is rejected, not cropped or
distorted. `document.rs` maps the retained pixels into the 32-row frame without
resampling feed columns; `Raster::pack_columns` packs them without resizing.
This is a software geometry contract, not a physical-size guarantee.
Historical lead-in estimates (including about 3.4 mm beyond blank raster
columns) are observations of particular labels, not fixed firmware geometry.

Some existing `geometry.rs` comments still describe 160 dpi as measured native
pitch. Those comments remain unchanged in this documentation-only update and
are not validated by the investigation.

## 7. Feed, margins, and completion

`ESC p 30` is not advertised here as automatic cutting capability. S2 calls it
CUT; the LetraTag has a manually operated cutter. Its meaning and effect on
feed remain unresolved. `ESC E` appears only in the legacy profile. No
undocumented configuration is implemented.

S2 `LT_200B.json5:55-87` adds 6 mm or 47 columns of trailing feed at 200 dpi
and reports an alternating 1/5 pattern for tiny labels. This third-party
observation is documented, not automatically adopted as immutable device
geometry. Our default adds no hidden extra feed. Physical cut length, blank
tape before/after content, and minimum reliable feed length are acceptance checks.

## 8. Errors and reconnection

BlueZ errors and timeouts lead to a controlled disconnect. Before the first
write, a new, explicitly submitted job is possible. Once a write begins, an
error, timeout, invalid response, or process termination leaves the outcome
uncertain. The persistent inflight entry blocks further jobs. No automatic
retransmission, including after a restart or reconnect.

Disconnect timing on hardware: the connection is dropped only after the
status phase ends (result notification or timeout), never while the printer
may still be physically printing. Disconnecting on the first (progress)
notification aborts an in-progress print; see section 3.

D claims that disconnecting discards partial jobs. This is not hardware-confirmed
here and does not justify retrying a job that may already have printed. No
firmware update, reset, density, or cassette-setting commands are sent. Status
queries outside a print job remain unimplemented because their specific safe
sequence is not established.

## 9. Firmware differences

Differences between public implementations are documented: name filters,
UUID tail handling, 01/81, form feed/ESC p, copy counts, and status interpretation.
No reliable firmware version → profile property table is available. Presenting
these differences as specific firmware versions would be invented. Phase 4
records firmware identifiers only if the actual GATT profile provides them;
missing identifiers are not replaced with a guessed DIS.

## 10. Reuse and licensing

The implementation uses MIT sources as interoperability references. Notices
and the corresponding MIT texts are in `docs/legal/`. AGPL code from S3 is
neither ported nor embedded in reference data. This standalone project is
MIT-licensed; native libraries retain their own license terms. Binary
redistribution is subject in particular to PAPPL's Apache-2.0/NOTICE requirements
and the terms of the dynamically used system libraries.
