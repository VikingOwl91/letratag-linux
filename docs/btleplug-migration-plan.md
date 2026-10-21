# Migration plan: btleplug BLE transport backend

Status: plan (pre-implementation), 2026-10-21.

## Objective

Add an established `btleplug` BLE transport backend alongside the existing
custom BlueZ (bluer) backend, behind the existing `Transport` abstraction and
a selectable CLI backend (`mock` | `bluez` | `btleplug`). The existing BlueZ
backend remains available for controlled comparison until migration
acceptance is complete. **Changing BLE libraries is not assumed to fix the
physical feed-pitch discrepancy; the migration is a complexity/observability
change with a secondary diagnostic goal.**

## Phase 1 findings (investigation summary)

### Existing architecture

* `letratag-protocol`: pure wire encoding (raster packing, legacy/modern
  commands, chunked writes, header/checksum, status parsing). No Bluetooth
  dependency. Unchanged by this migration.
* `letratag-render`: pure PNG/grayscale rendering. No Bluetooth dependency.
  Unchanged.
* `letratag-transport`: `Transport` trait (`connect`/`write`/`status`/
  `disconnect`/`write_short_command`/`parameters`), `ChunkMode`,
  `effective_write_limit` (min of transport limit, validated printer cap,
  protocol bound 503), `TransportParameters` diagnostics, `Error`,
  `MockTransport` (safe default), `BluezTransport` (feature `ble`).
* `letratag-printer`: `Engine` (serialization, journal, blocking, conservative
  outcomes), CLI (`preview`, `mock`, `serve`, `cups-settings`,
  `hardware-info`, `hardware-send`), PAPPL FFI (`TransportKind::{Mock,
  Bluez}`).

### BlueZ backend behavior to preserve (bluer 0.17.4)

* Adapter/device: default adapter; `resolve_device` reuses a known device or
  runs bounded LE discovery (15 s, 256 events, exact address match); reuses an
  existing link instead of failing on a second Connect.
* GATT profile: exactly one vendor service (`uuid >> 96 == 0xbe3dd650`);
  TX = service tail `…651`, RX = `…652` (notify), AUX = `…653` (writable).
  TX with only the `write` flag selects Write Request (ATT-acknowledged);
  `write-without-response` present selects Write Command (AcquireWrite).
  Profile mismatch (missing/ambiguous) is an error, never a guess.
* Write sizing: bluer-reported usable GATT value (495 on LT200B via
  mtu_workaround; reported as a characteristic capability, negotiated ATT MTU
  stays unknown); effective = min(transport, validated cap, 503). Validated
  cap 244 for `90:E5:B1:49:DA:AE` (operator-confirmed).
* Writes: sequential and awaited; Write Request; 3 s per-write timeout;
  `failed` set before awaiting so cancellation cannot enable replay; no
  automatic resend of accepted chunks.
* Status: next RX notification, 30 s timeout, 512-byte bound; interpretation
  unchanged (first 0/1 discarded as progress ack by the engine).
* Disconnect: 3 s timeout; handle retained until success.
* Short command (ESC M cassette): AUX characteristic, Write Request,
  best-effort, bounded by the ATT-level limit.
* Parameters diagnostics: transport, write mode, ATT MTU (unknown on
  LT200B), usable GATT value, validated cap, effective write size, chunk
  mode, experimental flag.

### Historical Python prototype (S1, `ysfchn/dymo-bluetooth`)

Recovered source (`requirements.lock`, `dymo_bluetooth/bluetooth.py`):
Bleak 1.1.1 (dbus-fast on Linux, i.e. still BlueZ). Print path:
`write_gatt_char(print_request, chunk, response=True)` — ATT Write Request,
same as the current Rust implementation; `start_notify` on the RX
characteristic before writing; 500-byte chunks from `command_print`;
scanner filtered by the vendor service UUID with a name check; existing
connections reused. **All historical and current implementations use ATT
Write Request for print data.**

### btleplug 0.11.8 (pinned)

* Rationale: current stable release (2026-10-10), maintained, permissive
  license (MIT/Apache-2.0/BSD-3-Clause), tokio-based (matches our runtime),
  Linux backend over BlueZ via `bluez-async` 0.8 (D-Bus), so it exercises the
  same OS Bluetooth stack through a different GATT/D-Bus client library.
* API: `platform::Manager::new()` → `adapters()` → `Central`
  (`start_scan(ScanFilter)`, `peripherals()`, `events()`, `adapter_info()`);
  `Peripheral` (`connect`, `disconnect`, `discover_services`, `write`,
  `read`, `subscribe`, `notifications` stream, `properties`).
* `WriteType::WithResponse` maps to bluez-async write type `request` (ATT
  Write Request); `WithoutResponse` maps to `command`.
* Linux limitation: `add_peripheral` is unsupported, so a known address must
  be found by scanning — the same discovery-resolution approach the BlueZ
  backend already uses.
* No negotiated-MTU or max-write-size API on Linux: the application-level
  validated cap (244) remains the effective limit; ATT MTU and the GATT
  value limit are reported as unknown rather than invented. The transport
  bound for validation is the protocol bound (503).
* `CharPropFlags` (WRITE 0x08, WRITE_WITHOUT_RESPONSE 0x04, NOTIFY 0x10)
  enables the same characteristic validation as the bluer flags.

## Design

### Feature flags (both backends buildable together for comparison)

* `letratag-transport`: `ble = ["dep:bluer", "dep:futures-util"]` (unchanged),
  new `btleplug = ["dep:btleplug"]`.
* `letratag-printer`: `ble = ["letratag-transport/ble", "dep:bluer",
  "dep:futures-util"]` (unchanged), new `btleplug =
  ["letratag-transport/btleplug", "dep:btleplug"]`.
* Defaults unchanged: no features = mock only. `btleplug` is pinned with
  `version = "=0.11.8"`.

### Backend selection (CLI, existing conventions)

* `serve --transport mock|bluez|btleplug` (`TransportChoice` gains
  `Btleplug`; `--address` required for both real backends; no implicit
  fallback).
* `hardware-send --transport bluez|btleplug` (new option; default `bluez`
  preserves current behavior).
* `ffi::TransportKind::{Mock, Bluez { .. }, Btleplug { .. }}` with per-variant
  `unsupported()` messages when a feature is not compiled in.
* `hardware-info` stays on the BlueZ backend (unchanged).

### btleplug transport (`crates/letratag-transport/src/btleplug.rs`)

* `BtleplugTransport` implements the existing `Transport` trait unchanged.
* Mockable GATT boundary: public async traits `GattCentral` (adapter info,
  bounded scan/find by exact address) and `GattPeripheral` (connect,
  disconnect, discover, services/characteristics, write with mode, subscribe,
  notifications, properties). The real implementation wraps btleplug platform
  types; tests use a scripted mock. All transport logic (profile resolution,
  chunk sizing, write loop, status, timeouts, parameters) is generic over the
  boundary, so it is deterministically testable without hardware.
* Connect: manager → first adapter (error if none; adapter_info recorded) →
  bounded scan (15 s, 256 events, exact address match; `DeviceNotFound`
  otherwise) → `connect` (10 s) → `discover_services` → validate exactly one
  vendor service → resolve TX/RX/AUX by tail with `CharPropFlags` validation
  (same mode preference as the BlueZ backend; LT200B selects WithResponse) →
  subscribe RX → compute effective write size.
* Writes: sequential, awaited, selected `WriteType`, 3 s timeout, `failed`
  set before awaiting, no replay, 256-write limit, per-write size bound.
* Status: notification stream filtered to the RX UUID, 30 s timeout,
  512-byte bound.
* Disconnect: 3 s timeout.
* Short command: AUX characteristic, same write mode, best-effort.
* Parameters: transport `btleplug`, write mode, ATT MTU unknown, GATT value
  limit unknown (not exposed), validated cap, effective write size, chunk
  mode, experimental flag.
* Symmetric `describe_gatt()` diagnostics on both real backends for the
  controlled comparison.

### Preserved guarantees

* `letratag-protocol` and `letratag-render` untouched (no BLE deps).
* Mock remains the default transport; `serve` without `--transport` is mock.
* Engine state machine, journaling, blocking, uncertain outcomes and
  operator-decision workflows unchanged.
* Direct IPP remains the recommended workflow; CUPS remains Experimental.
* No DPI, geometry, protocol or rendering changes.

## Phase 4 test plan (deterministic, hardware-independent)

Fixtures (committed under `crates/letratag-transport/tests/fixtures/`):
the retained HELLO LINUX legacy capture and the RAW diagnostic capture.

* Payload preservation: `Job::new(raster, Legacy).writes(244)` reproduces the
  exact HELLO LINUX wire capture (body 1624, writes `[9,242,242,242,242,242,242,181]`,
  body SHA-256 `58272474…`, wire SHA-256 `357a6b8d…`) and the RAW diagnostic
  capture (body SHA-256 `0e2f7648…`, wire SHA-256 `2720fc13…`).
* Mock-GATT transport tests: exact write sequence and `WithResponse` mode;
  chunk boundaries; write failure at index N (no retry, error surfaces);
  mid-transfer disconnect; status timeout; notification filtering; profile
  validation (missing/ambiguous service, wrong properties); discovery
  timeout (`DeviceNotFound`); missing adapter; parameters reporting;
  engine integration (inflight journal + blocked after uncertain outcome).
* Existing suites unchanged: Direct IPP, PDF-only IPP, geometry, protocol,
  engine, renderer.

## Phase 5 comparison plan (no physical print)

Connect-only comparison of both backends against the real printer (no TX
writes, no print payload): adapter identity, device properties, GATT table
with properties, selected characteristics and write mode, computed payload
SHA-256 and write plan (identical by test), one bounded status attempt,
connect/disconnect behavior and timings. Differences are investigated before
any physical print.

## Phase 6 hardware acceptance (requires explicit operator authorization)

Exactly one `HELLO LINUX` print via `hardware-send --transport btleplug`
with the retained PNG and the byte-identical historical payload. Operator
measures readability, orientation and text width. ~12.3 mm: btleplug
reproduces current behavior (library change alone does not resolve the
discrepancy). ~24.3 mm: backend-dependent difference indicated — compare
operation traces before assigning a cause. Other: investigate without
assuming a mechanism. No DPI compensation or protocol changes.

## Phase 7 decision

Adopt / retain both / retain BlueZ, based on regression results, hardware
acceptance, and operational trade-offs. The original backend is not removed
solely because the new one compiles or prints.
