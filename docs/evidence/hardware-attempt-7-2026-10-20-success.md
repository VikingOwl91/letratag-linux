# Hardware attempt 7, 2026-10-20: FIRST SUCCESSFUL PHYSICAL PRINT

First verified physical label from the DYMO LetraTag 200B
(`90:E5:B1:49:DA:AE`) through the existing driver architecture:
PNG -> rasterization -> encoding -> BLE transport -> physical label.
Operator-confirmed: `HELLO LINUX` readable on the label, orientation correct.

## Command and journal

```sh
target/debug/letratag-printer hardware-send test-output/hello-linux.png \
  --address '90:E5:B1:49:DA:AE' \
  --state "$PWD/test-output/hardware-7" --columns 400 \
  --profile legacy --max-write 244 --linger-seconds 20 \
  --acknowledge-unverified-hardware --diagnostics
```

Fresh journal `test-output/hardware-7`; diagnostics in
`test-output/hardware-7-diag.log`. The inflight entry was archived with the
operator decision after confirmation (see `test-output/hardware-7/archive/`).

## Diagnostic trace

```
[diag] connected; usable value size 244 at 4.36s
[diag] write 1/8 len=9 at 4.39s            (header)
[diag] write 2/8..7/8 len=242 ...          (6 chunks of 241 body bytes)
[diag] write 8/8 len=181 at 5.18s          (final chunk)
[diag] status [1b 52 00] at 12.42s         (7.24 s after the last write)
[diag] outcome Err(Uncertain) at 42.42s
print outcome uncertain; operator inspection required, no automatic retry
```

The `1b 52 00` (success) notification arrived 7.24 s after the last write.
That matches the physical print duration: 400 feed columns at the measured
~160 dpi feed pitch = 63.5 mm; at the source-claimed ~7 mm/s print speed
that is ~9 s, at the observed pace ~8.8 mm/s. The result notification
arrives **after** physical printing completes.

The engine discarded the `00` as a progress ack (S1 two-message model) and
waited for a second notification that never came; the 30 s status read
timeout then held the connection open past the print and returned
Uncertain at 42.42 s, disconnecting at ~45 s. The `--linger-seconds 20`
flag did not execute (the status-loop timeout propagated first). The label
was already fully out when the success notification arrived.

## Operator confirmation and measurements

Operator report (2026-10-20): `HELLO LINUX` readable on the label,
orientation correct. The device and tape are new and freshly unpacked
(operator). Ruler measurements along the feed:

| Segment  | Measured | Raster content                | Implied pitch   |
|---|---|---|---|
| Vorlauf (leading blank) | 23.25 mm | columns 0..123 (124 blank) | ~135 dpi incl. lead-in |
| Text     | 24.30 mm | columns 124..275 (152 ink)  | **158.9 dpi**   |
| Nachlauf (trailing blank) | 19.0 mm | columns 276..399 (124 blank) | ~166 dpi (rounding) |
| Total    | 66.55 mm | 400 columns                 | 152.7 dpi incl. lead-in |

Ink extent in the raster: columns 124..275, rows 8..23 (16 rows).

## Findings

1. **Physical printing works.** Verified end to end on real hardware,
   operator-confirmed. This is the primary acceptance result.
2. **Feed dot pitch is ~160 dpi, measured 158.9** from the text segment
   (152 columns = 24.3 mm). This matches the manufacturer product-page spec
   (160 DPI) and contradicts the 200 dpi development layout value used by
   S1/S2 and this project. Consequence: content laid out at 200 dpi prints
   200/160 = 1.25x longer than intended along the feed. The provisional
   PDF geometry (30-row content area at 200 dpi = 3.81 mm) is mis-scaled
   for this device; the correct content area at 160 dpi is 4.76 mm.
   The across-head pitch was not measured in this run (text height is
   16 rows; the user measured feed-direction lengths only).
3. **The printer adds a lead-in.** The measured leading blank (23.25 mm)
   exceeds the raster's leading blank columns (124 columns = 19.7 mm at
   160 dpi) by ~3.4 mm (~21 columns). Recorded as observed chassis
   behavior; the exact mechanism (fixed feed before content vs.
   measurement baseline) is not established.
4. **Trailing blank prints as raster content.** The measured trailing
   blank (19.0 mm) matches the raster's trailing blank columns (124
   columns = 19.6 mm at 160 dpi). No hidden extra feed was added by the
   printer for this legacy job.
5. **Root cause of attempts 1-6: weak batteries.** With the old cells the
   printer fail-fast rejected jobs (status `1b 52 02`, 0.3 s after
   receipt; motor blip and fast LED blink observed by the operator). The
   operator replaced the batteries with well-charged ones before attempt
   7; device and tape are new. Same configuration then printed. The
   profile/chunk-size/ESC-M experiments in attempts 2-6 were confounded
   by the weak batteries and are inconclusive as to those variables;
   legacy profile + 244-byte chunks is the confirmed working
   configuration, not a proven-optimal one.
6. **Terminal status contract (observed once):** on this firmware the
   successful job produced a single `1b 52 00` notification after the
   physical print. The S1 two-message model (first PRINTING ack, then
   result) did not match the successful case. One observation; the
   completion policy is deliberately unchanged (still uncertain, inflight
   journal) until calibrated with repeated jobs and failure cases.

## Remaining acceptance items

* Terminal-status calibration: repeated jobs, failure cases with fresh
  batteries (lid open, empty cassette behavior), disconnect timing after
  the result notification. Then update the completion policy and tests.
* Across-head dot pitch (measure printed text height).
* Effective DPI confirmation across substrates; 160 vs 200 dpi resolution
  of the layout value for the PDF profile.
* CUPS/PAPPL submission over BLE (serve path is still mock-only);
  separate approval required.
* The 60-second host adapter power cycle (blueman-applet + USB autosuspend
  on the Realtek dongle) remains an environmental risk for transfers.
