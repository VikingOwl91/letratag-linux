# Hardware calibration batch, 2026-10-20: repeatability and geometry

Two further successful physical prints (jobs A and B), byte-identical to
attempt 7 (same PNG, same 400-column raster, legacy profile, max-write 244).
Purpose: confirm repeatability of the status sequence and measure the
across-head dot pitch. Operator confirmed both labels: `HELLO LINUX`
readable, orientation correct.

## Diagnostic traces (both jobs)

```
Job A (hardware-8): writes done 5.07s, status [1b 52 00] at 12.23s (+7.16s),
                    outcome Uncertain at 42.23s (30s status timeout)
Job B (hardware-9): writes done 5.60s, status [1b 52 00] at 12.73s (+7.13s),
                    outcome Uncertain at 42.73s (30s status timeout)
```

Third and fourth consistent observation of the sequence: exactly one result
notification `1b 52 00` (success), arriving after the physical print
duration. No 1/5 alternation (S2 bench quirk) with fresh batteries and
400-column content. Both journals archived with the operator decision
"physically completed, operator-confirmed".

## Measurements (operator, ruler; one caliper misread corrected)

| Segment | Attempt 7 | Job A | Job B | Raster content | Expected @160dpi |
|---|---|---|---|---|---|
| Vorlauf (leading blank) | 23.25 mm | 19.0 mm | 18.5 mm | 124 blank cols | 19.7 mm |
| Text width (feed) | 24.3 mm | ~24 mm (corrected) | ~24 mm (corrected) | 152 ink cols | 24.1 mm |
| Text height (across) | not measured | 2.25 mm | 2.5 mm | 16 ink rows | 2.54 mm |
| Nachlauf (trailing blank) | 19.0 mm | 18.65 mm | 18.5 mm | 124 blank cols | 19.7 mm |
| Total label | 66.55 mm | ~62 mm | ~62 mm | 400 cols | 63.5 mm |

The operator initially read the text width as ~12 mm on an analog caliper
(misread, corrected; attempt 7's 24.3 mm is the consistent value). Attempt
7's longer Vorlauf (23.25 mm) is a one-time fresh-tape advance (~4 mm);
jobs A/B ran on already-advanced tape and match the raster's leading blank
columns exactly.

## Findings

1. **Repeatability confirmed.** Three successful prints (attempt 7, A, B),
   all with the same status sequence and consistent physical output.
2. **Dot pitch is ~160 dpi isotropic.** Feed: 152 text columns = 24.3 mm =
   158.9 dpi. Across: 16 text rows = 2.25-2.5 mm (2.54 mm expected at 160
   dpi; within caliper precision). Total label: 400 columns = ~62 mm. This
   matches the manufacturer's 160 DPI spec on both axes. The project's
   200 dpi development layout value is 25% too large along the feed and
   must be recalibrated before the PDF geometry is trusted.
3. **Status contract (4 observations):** a successful job produces exactly
   one `1b 52 00` notification, after the physical print duration. The S1
   two-message model (first PRINTING ack, then result) does not match this
   firmware's successful jobs. The engine's conservative handling (discard
   the first 0/1 notification, end the status phase on a later notification
   or timeout, never claim completion) remains safe: the print is complete
   by the time the result arrives, and the 30 s status timeout covers the
   disconnect. The completion policy is deliberately unchanged; see
   hardware-acceptance.md.
4. **No 1/5 alternation** with fresh batteries and 400-column content
   (consistent with S2's bench note that wider content avoids it).
