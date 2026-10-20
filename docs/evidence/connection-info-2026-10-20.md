# Connection info-extraction pass, 2026-10-20

Goal: reverse-engineer what information is available through the Bluetooth
connection to the LetraTag 200B (`90:E5:B1:49:DA:AE`). One pass with a
diagnostic script (bleak over BlueZ): full GATT dump, notifications
subscribed on RX and AUX, state reads around the documented ESC M
short command and a byte-identical print job, all notifications captured.
The job printed successfully (result `1b 52 00` after the print duration).

## Full GATT dump (fresh connection)

| Handle | UUID | Props | Value |
|---|---|---|---|
| svc 0x0001 | 0x1801 GATT | primary | |
| 0x0002 | 0x2a05 Service Changed | indicate | CCCD 0x0004 = `0200` (indications enabled) |
| svc 0x0014 | 0x1800 GAP | primary | |
| 0x0015 | 0x2a00 Device Name | read | `Letratag` |
| 0x0017 | 0x2a01 Appearance | read | `0000` |
| 0x0019 | 0x2aa6 (unknown SIG char) | read | `00` (unchanged in all states) |
| svc 0x0028 | 0x180a DIS | primary | |
| 0x0029 | 0x2a29 Manufacturer Name | read | `Newell` |
| 0x002b | 0x2a24 Model Number | read | `LT200B` |
| 0x002d | 0x2a25 Serial Number | read | `CRGHEQNVAA` |
| 0x002f | 0x2a27 Hardware Revision | read | `REV-E` |
| 0x0031 | 0x2a26 Firmware Revision | read | `RUTEP20220628` |
| svc 0x0033 | be3dd650 vendor | primary | |
| 0x0034 | be3dd651 TX | write (Write Request) | not readable |
| 0x0036 | be3dd652 RX | read, notify | CCCD 0x0038 = `0000` until subscribed |
| 0x0039 | be3dd653 AUX | read, write, notify | CCCD 0x003b = `0000` until subscribed |

No Battery Service (0x180f) is present. No other services.

## RX characteristic (be3dd652): status channel

The cached value reflects the job status; the printer pushes the final
result as a notification.

* Idle: `1b 52 00`.
* During a print job: `1b 52 01` (code 1 = PRINTING, confirming S1's table;
  visible via read; S2's note that the characteristic can be polled during
  printing for a progress UI is confirmed).
* After a successful print: `1b 52 00` (code 0), pushed as a notification
  ~7.1 s after the last write (the physical print duration).
* Notifications also fire on reads (BlueZ re-delivers the cached Value as a
  PropertiesChanged notification after a ReadValue when notifications are
  enabled). Most RX notifications in this pass were read-triggered
  re-deliveries; the job result was a genuine push.

## AUX characteristic (be3dd653): short-command + status/echo channel

* Readable 4-byte value: `00 00 00 00` at idle, `01 00 00 00` after the
  ESC M (set-cassette-type, id 3) short command, stable through and after
  the print job. The first byte changes 0 -> 1 in response to the ESC M
  command; the meaning of `01` is not established (candidates: command
  acknowledged, media-type-configured flag). Bytes 2-4 stayed `00`.
* Writable (short commands; the ESC M write was ATT-acknowledged).
* Notify-capable; the value is pushed/re-delivered like RX.
* No spontaneous AUX notifications outside reads and the post-ESC-M change.

## GAP 0x2aa6

Readable, value `00` in every observed state (idle, after ESC M, during and
after a job). No state information extracted; role remains unidentified.

## What is NOT available (state broadcast absent on this firmware)

* No ManufacturerData / ServiceData in advertising (verified across many
  scans; BlueZ device cache shows none).
* No Battery Service (0x180f) in GATT.
* No spontaneous notifications in a 15 s idle watch (0 events): no battery,
  lid, cassette or jam push channel.
* S2's documented 3-byte advertising state payload (cassetteId, battery
  level 0-3, low/too-low flags, TAPE_JAM/CUTTER_JAM, busy, charging) is
  NOT broadcast by this firmware (RUTEP20220628). Battery, lid and tape
  state are therefore not directly extractable from this device.

## On/off and availability

* The printer advertises when powered on and idle (passive scan shows it);
  it stops being discoverable at least when the lid is open (Job C, see
  [hardware-failure-cases-2026-10-20.md](hardware-failure-cases-2026-10-20.md)).
* A direct connect to the cached address works even when a scan does not
  see it (BlueZ Device1.Connect).
* After a disconnect, an immediate reconnect can fail; a few seconds of
  settle time avoid it (observed SafeTransport retries).

## Print job in this pass

8 writes (header 9 bytes, 6 chunks of 242 bytes, final 181 bytes), legacy
profile, max-write 244, byte-identical to the validated CLI stream. All
writes ATT-acknowledged. Result notification `1b 52 00` 7.1 s after the
last write. The label printed (operator to confirm/cut).

## Summary: extractable per state

| Info | Available? | Channel |
|---|---|---|
| Device identity (mfr/model/serial/HW/FW) | yes | DIS reads |
| Job status (idle/printing/result) | yes | RX read + notify |
| Short-command ack / echo (0->1) | yes | AUX read + notify |
| On/off | yes | advertising presence / connect |
| Battery level / state | **no** | (no broadcast, no Battery Service) |
| Lid state | indirect only | advertising presence (unreliable) |
| Tape/cassette presence | **no** | (code 7 documented, never observed) |
| Jam/busy flags | **no** | (not broadcast on this firmware) |
| Unknown GAP 0x2aa6 | read `00` | no state info |
