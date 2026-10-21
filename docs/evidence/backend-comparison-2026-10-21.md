# Backend comparison: BlueZ vs btleplug (connect-only), 2026-10-21

Comparison of the existing BlueZ (bluer) backend and the new btleplug
backend before any physical print. Method: `backend-compare` example
(`crates/letratag-printer/examples/backend-compare.rs`), run with both
features enabled. **No TX writes; no print payload was transmitted.** Each
backend connected, reported parameters, dumped its GATT table, made one
bounded status attempt, and disconnected. Full log:
`test-output/backend-comparison-055004.log` (timestamps in UTC).

## Reference payload (identical for both backends by construction)

Rendered from the retained `test-output/hello-linux.png` with the original
renderer settings, legacy profile, validated 244-byte cap:

* frame 400 x 32; body 1624 bytes; 8 writes `[9,242,242,242,242,242,242,181]`
* body SHA-256 `58272474123e691ac5d206d11cc91c677efc1b5119a346bb1c7451514d069c53`
* wire SHA-256 `357a6b8dad5a11ca68a39b9825cee3d91551c195f69d7065e07cef0df0eeb98b`

Both match the retained HELLO LINUX reproduction payload exactly
(byte-preservation also covered by `tests/payload_preservation.rs`).

## Per-backend results

| Observation | BlueZ (bluer) | btleplug |
|---|---|---|
| Adapter | hci1 | hci1 (usb:v1D6Bp0246d0557) |
| Device | 90:E5:B1:49:DA:AE | 90:E5:B1:49:DA:AE, name "Letratag" |
| Connect | 1.77 s | 0.75 s |
| Write mode | Request | Request |
| Negotiated ATT MTU | unknown | unknown |
| Usable GATT value | 495 | unknown (not exposed by btleplug on Linux) |
| Validated printer cap | 244 | 244 |
| Effective write size | 244 | 244 |
| Chunk mode | auto | auto |
| Vendor service | be3dd650-2b3d-42f1-99c1-f0f749dd0678 | same |
| TX be3dd651 | write=true, w/o-response=false, notify=false | WRITE |
| RX be3dd652 | write=false, w/o-response=false, notify=true | READ \| NOTIFY |
| AUX be3dd653 | write=true, w/o-response=false, notify=true | READ \| WRITE \| NOTIFY |
| Status attempt (idle) | 30.00 s timeout | 30.00 s timeout |
| Disconnect | Ok in 2.40 s | Ok in 2.25 s |

Both backends discovered the same 12 characteristics across the same four
services, selected the same TX/RX/AUX roles from the same properties, and
chose the same write mode (ATT Write Request). The status behavior when idle
is identical (no notification; bounded 30 s timeout).

## Differences and assessment

1. **GATT value limit reporting**: BlueZ reports 495 (a characteristic
   capability via bluer's mtu_workaround); btleplug exposes no such API on
   Linux, so it is reported unknown. The effective write size is 244 in
   both (validated printer cap), so print behavior is unaffected.
2. **Connect/disconnect timing**: btleplug connected faster (0.75 s vs
   1.77 s) and disconnected slightly faster (2.25 s vs 2.40 s). No
   correctness impact; timing diagnostics remain observable.
3. **No differences in**: device selection (exact address + GATT profile
   validation), write mode, command ordering, chunk boundaries, payload
   bytes, status interpretation, or safety semantics (timeouts, no retries,
   uncertain outcomes).

**Assessment**: the backends are operationally equivalent at the application
and GATT-operation level. Nothing in this comparison explains or predicts a
difference in physical feed pitch. The secondary diagnostic goal (does the
BLE transport implementation contribute to the apparent feed-pitch
discrepancy?) is answered at this level: **no application-level difference
was found**; the remaining difference is the D-Bus client library
(bluer vs bluez-async) and its timing, which the single hardware acceptance
print (Phase 6) tests directly.

No link-layer (air traffic) captures exist; this comparison covers
application-level payloads and GATT operation traces only.
