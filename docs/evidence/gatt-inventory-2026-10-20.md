# LetraTag 200B hardware discovery evidence, 2026-10-20

Read-only observations only. No pairing, no GATT writes, no print job were
performed. The device was connected twice for service resolution and DIS
reads, then disconnected. Device state after discovery: trusted, not paired,
not connected (unchanged from before).

## Environment

* Host adapter: `8C:88:4B:26:A5:CF` ("cn-arch", default). BlueZ 5.87.
* Adapter D-Bus path was `/org/bluez/hci6` at first inspection and
  `/org/bluez/hci2` after the adapter was re-enumerated (power cycle during
  an unrelated scan attempt). The adapter index is **not stable**; the Rust
  transport uses BlueZ's default adapter and is unaffected.
* `btmon` could not bind the monitor channel (`Operation not permitted`), so
  no HCI-level trace was captured. ATT MTU evidence comes from the BlueZ
  `GattCharacteristic1.MTU` property instead.

## Device identity (BlueZ Device1, verified live)

* Address: `90:E5:B1:49:DA:AE`, AddressType: `public`
* Advertised/cached name: `Letratag 90E5B149DAAE`; GATT Device Name: `Letratag`
* Trusted: yes. Paired: no. Bonded: no. Blocked: no.
* Connection succeeds **without pairing**; services resolve while unpaired.
* Advertised UUIDs: `0000180a` (Device Information),
  `be3dd650-2b3d-42f1-99c1-f0f749dd0678` (vendor).

## Device Information service (0x180a), values read over GATT

| Characteristic | UUID | Value (ASCII) |
|---|---|---|
| Manufacturer Name String | 0x2a29 | `Newell` |
| Model Number String | 0x2a24 | `LT200B` |
| Serial Number String | 0x2a25 | `CRGHEQNVAA` |
| Hardware Revision String | 0x2a27 | `REV-E` |
| Firmware Revision String | 0x2a26 | `RUTEP20220628` |

GAP service (0x1800): Device Name `Letratag` (0x2a00), Appearance `0x0000`
(0x2a01), and one further read characteristic 0x2aa6 returning `00`
(role not identified).

Correction 2026-10-20 (connection-info pass): the Hardware/Firmware Revision
strings above were re-read as hex during the connection-info pass
([connection-info-2026-10-20.md](connection-info-2026-10-20.md)) and are
`REV-E` and `RUTEP20220628`. The first decimal decode of those two strings
was a transcription error; the hex reads are authoritative.

## Full GATT inventory (handles observed on 2026-10-20)

| Handle | UUID | Flags (BlueZ strings) | Notes |
|---|---|---|---|
| svc 0x0001 | 0x1801 Generic Attribute | primary | GATT |
| 0x0002 | 0x2a05 Service Changed | indicate | CCCD 0x0004 (cached `02 00`) |
| svc 0x0014 | 0x1800 Generic Access | primary | GAP |
| 0x0015 | 0x2a00 Device Name | read | `Letratag` |
| 0x0017 | 0x2a01 Appearance | read | `00 00` |
| 0x0019 | 0x2aa6 | read | `00`, role unknown |
| svc 0x0028 | 0x180a Device Information | primary | DIS |
| 0x0029 | 0x2a29 Manufacturer Name | read | `Newell` |
| 0x002b | 0x2a24 Model Number | read | `LT200B` |
| 0x002d | 0x2a25 Serial Number | read | `CRGHEQNVAA` |
| 0x002f | 0x2a27 Hardware Revision | read | `REV-E` |
| 0x0031 | 0x2a26 Firmware Revision | read | `RUTEP20220628` |
| svc 0x0033 | be3dd650-2b3d-42f1-99c1-f0f749dd0678 | primary | DYMO vendor print service |
| 0x0034 | be3dd651-2b3d-42f1-99c1-f0f749dd0678 | **write** | TX: only Write Request (with response). **No write-without-response.** |
| 0x0036 | be3dd652-2b3d-42f1-99c1-f0f749dd0678 | read, notify | RX: CCCD 0x0038 (`00 00`, notifications off). Cached value `1b 52 00` = `ESC R 0` (status code 0) |
| 0x0039 | be3dd653-2b3d-42f1-99c1-f0f749dd0678 | read, write, notify | auxiliary: CCCD 0x003b (`00 00`). Cached value `00 00 00 00` |

BlueZ flag strings: `write` = Write Request/Response (ATT Write Request,
acknowledged), `write-without-response` = Write Command (unacknowledged).

## Connection parameters

* `GattCharacteristic1.MTU` reports 500 for all characteristics (BlueZ MTU
  property; bluer applies its five-byte workaround on top). This is the
  negotiated per-characteristic value size, not the raw ATT MTU.
* Exactly one service matches the vendor service UUID; TX/RX UUIDs match the
  `...d650` tail +1/+2 derivation used by the transport.

## Consequences for the implementation

1. The TX characteristic exposes only `write` (Write Request). The transport
   previously required `write-without-response` and used AcquireWrite; that
   would reject this hardware with `Error::Profile`. The transport now accepts
   either write mode and prefers Write Command (AcquireWrite) when offered.
2. The RX characteristic matches the implementation (notify + CCCD). Enabling
   notifications writes the CCCD and is a GATT write: it requires operator
   approval before execution.
3. The RX characteristic is also readable and caches the last status
   (`1b 52 00` observed while idle). Notifications remain the primary status
   channel; the readable cache is an observation, not an adopted mechanism.
4. No pairing is required for connection or service resolution.

## Baseline offline verification (same day, before any code change)

* `cargo test --locked --workspace`: 47 passed, 0 failed.
* `cargo test --locked --workspace --all-features`: 54 passed, 0 failed.
