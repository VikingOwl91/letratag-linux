# Independent geometry forensics, 2026-10-21

## Result and limits

**The new artwork shrink is introduced by CUPS `pdftopdf`, before this
application rasterizes or packs it.** The original artwork streams are
byte-identical. With the submitted `media=custom_12x63.5mm` and no explicit
scaling option, the installed filter selects a 12 x 50 mm output page and
embeds a rotation combined with **0.787402 scaling on both axes**. The
resulting raster has 315 columns, not 400. Its raster SHA-256 exactly
matches the preserved CUPS mock reproduction from after job 48.

This explains why both physical rectangle dimensions shrank while label
length stayed constant. It does **not** demonstrate variable feed stepping.
It does **not** resolve the older Oct 20 versus Oct 21 apparent feed-pitch
conflict. No production rendering, protocol, transport, version, or release
changes were made. No further physical print was submitted. The remaining
BlueZ serving process was stopped during this investigation; the inflight
journal and system queues were left intact.

Important limit: **there are no retained raw hardware payload captures for
these calibration prints.** Hardware diagnostics preserve lengths and
timings, not bytes. All `.ble` files below are mock/local captures. Hardware
fields inferred from those lengths are marked derived, not observed bytes.
The historical submitted/filtered PDFs were not recovered from the protected
CUPS spool. Current offline reconstruction is strong corroboration, not a
retroactive hardware packet capture.

The earlier unreviewed report is preserved as
`geometry-2026-10-21/previous-unreviewed-report.txt`, not endorsed. Its
claims that batteries/mechanics were ruled out, that feed pitch changed,
and that mock raster verification proved the transmitted artwork were
unsupported. The same cautions apply to historical journal decision prose.

## Evidence and definitions

Artifacts: [geometry-2026-10-21/](geometry-2026-10-21/).
`originals.json` records source paths, sizes, timestamps, and SHA-256 hashes;
`offline-artifacts.json` records the subsequent filter controls.
`decoded.json` contains complete capture/body/raster hashes and byte fields.
The independent decoder is `scripts/forensic-raster.py`; it imports no
project encoder/decoder. Its hand-constructed asymmetric fixture checks
orientation, byte order, both profiles, corruption, and truncation.

- **Observed:** bytes read from a retained artifact or a new offline run.
- **Operator:** reported physical measurement/state, not independently measured here.
- **Derived:** arithmetic or reconstruction, conditional on identified input/settings.
- **Unknown:** missing evidence; not filled in from assumptions.

Feed is the long axis along the tape. Across is perpendicular to feed.
The tape is 12 mm wide. That is not the printable band, the 32-row wire
frame, or the cut-to-cut length. A raster pixel's occupied extent is one
pixel wide; the separation between the first and last pixel centers is
one pixel smaller. Both conventions are recorded, not mixed silently.

## Chronology

Times are UTC from journal identifiers, whose creation uses wall-clock
nanoseconds; they are not independently synchronized printer timestamps.
For hardware rows, original transmitted bytes and checksums are **unknown**.
The input/capture association is reconstructed from invocation records,
retained artifacts, and the current offline rendering match.

| Session/job | Input, page and render | Wire/raster evidence | Operator/state evidence |
|---|---|---|---|
| Oct 20 attempts 1-6, 18:15-19:36 | `hello-linux.png`, 1000 x 80 px; no physical page/DPI encoded; PNG fit into 400 x 32 frame, active rows 30, threshold 128, no dither | Attempt 1 incomplete record; 2 modern, 3-6 legacy according to invocation records. Logs 2-5: 97 writes at cap 20 (18-byte non-final chunks); 6: 8 writes at cap 244. No hardware raster retained | No successful label reported. Weak-cell explanation is historical, not an isolated proof of all failures |
| Oct 20 attempt 7, 19:44:24 | Same PNG and PNG rendering settings | Log: `[9,242 x6,181]`, body 1624, width 400 under legacy model. Retained **mock proxy**: 400 x 32, ink x124..275/y8..23 (152 x 16 occupied pixels), raster hash `76365e...1927` | Fresh/well-charged batteries reported, new tape/device. Feed ink 24.30 mm, lead 23.25 mm, trail 19 mm, total 66.55 mm; across not measured |
| Oct 20 A/8, 20:01:24 | Same input/settings per invocation record | Same logged lengths; no job-specific raster bytes | Readable label. Lead 19 mm, feed ink about 24 mm (corrected historical reading), height 2.25 mm, trail 18.65 mm, total about 62 mm |
| Oct 20 B/9, 20:03:24 | Same input/settings per invocation record | Same logged lengths; no job-specific raster bytes | Lead 18.5 mm, feed ink about 24 mm (corrected historical reading), height 2.5 mm, trail 18.5 mm, total about 62 mm |
| Oct 20 C/10 through G/14 | Same PNG per records; no rectangle | 10/12 logs say transport failed before transmission; 11 stops at write 4/8; 13/14 complete 8-write traces | 13 readable recovery label, no dimensions. 14 lid open, no output despite `00`. Docs claim 12 phantom-success transmission, but its retained log contradicts that claim; unresolved record conflict |
| Oct 21 direct IPP acceptance, 00:19:00 | `asset-0001.pdf`, 50 x 12 mm, PDF render 160 dpi | Legacy log supports 315 columns; direct/mock proxy available, no job-specific payload | Readable label; no dimensional measurements |
| Oct 21 CUPS job 43, 01:08:11 | `cups-e2e-0001.pdf`, 50 x 12 mm, named 12x50 mm media, PDF 160 dpi | Seven-write legacy trace; no job-specific payload | Readable, oriented, uncropped, no duplicates; no dimensional measurements |
| Oct 21 job 44, 02:06:35 | `calibration-160.pdf`, 50 x 12 mm, named `12x50mm.Borderless`, PDF 160 dpi | Seven writes `[9,242 x5,82]`, legacy body 1284, derived width 315. Direct mock 252-pixel horizontal stroke/30 occupied rows. Current CUPS reconstruction also 252/30, with a one-pixel edge-position change | Initially rectangle 20.25 x 4.75 mm, ticks about 5.25 mm, total 43.15 mm, lead 10 mm. Latest comparison reports 20.25 x **4.65** mm and trail 13.25 mm; do not treat these as identical readings |
| Oct 21 job 45 | Re-submission while engine blocked | No second transmission per historical record; not an additional label | No new measurement |
| Oct 21 job 46, 03:20:53 | Same 315 PDF/settings | Same lengths, no payload bytes | Operator: exact replica, not a separate set of caliper values |
| Oct 21 job 47, 03:29:01 | Same 315 PDF/settings, battery comparison | Same lengths, no payload bytes | Operator: exact replica after battery replacement. Type/voltages/replacement time not recorded. Later preparation explicitly describes rechargeable cells at charger 100%; do not backdate that detail |
| Oct 21 job 48, 03:39:14 (submitted 03:39:13) | `calibration-400.pdf`, **63.5 x 12 mm**, `media=custom_12x63.5mm`, scaling option omitted | Source mock 400 columns; hardware log still seven writes/body 1284, derived 315 columns. Preserved CUPS mock and current filter reproduction: **315 x 32**, horizontal stroke **199 pixels**, **24 occupied rows** | Charged rechargeable cells, charger 100%, cassette removed/reseated, printer ready. Rectangle **16.25 x 3.65 mm**, trail **17 mm**. Lead and cut-to-cut length reported identical to previous label; no new absolute values supplied |

Power cycles, voltages, battery chemistry for earlier sessions, precise
cassette-change times, and unreported per-label dimensions remain unknown.
Historical corrections to analog-caliper readings reduce confidence in
precise cross-session pitch inference; they do not justify inventing new
measurements.

### Transport comparison

Write Request is directly logged on Oct 21; Oct 20 Request mode follows
from the inspected GATT characteristic flags and corresponding code path,
not a mode field in those older logs. Cap 244 reserves 241 body bytes per
chunk. Oct 21 logs show GATT value limit 495, negotiated ATT MTU unknown.
The older logs do not establish either limit independently.

| Run | First/final write (seconds after engine entry) | Status `1b 52 00` | Status minus final write |
|---|---|---|---|
| 7 | 4.432281 / 5.182243 | 12.419415 | 7.237171 s |
| 8 | 4.320520 / 5.070520 | 12.232742 | 7.162222 s |
| 9 | 4.852656 / 5.601680 | 12.726900 | 7.125220 s |
| 13 | 1.150826 / 1.936858 | 9.099982 | 7.163124 s |
| 14, lid open | 1.220052 / 1.970071 | 9.133289 | 7.163219 s |
| 44 | 0.853246 / 1.492294 | 7.678714 | 6.186421 s |
| 46 | 0.834996 / 1.471981 | 7.660116 | 6.188135 s |
| 47 | 1.010107 / 1.649104 | 7.836313 | 6.187209 s |
| 48 | 0.859414 / 1.496872 | 7.649063 | 6.152191 s |

Successful writes are awaited sequentially; Request mode passes each slice
to `write_ext` without altering pixel data (`bluez.rs:280-317`). Failure
does not silently retry the job. Logs prove local call completion, not
printer-side parsing. No supported mechanism ties chunk boundaries to
uniform shrinking here: shrinking already exists in the offline PDF and
raster with no BLE. Equal write lengths do not prove equal contents.

## Where scaling enters

### 1. Original artwork: unchanged

Both PDFs have the same 785-byte content stream, SHA-256:

```text
cbe5f6635d750fd9345ef32873b187f7ef676c10620ae22ab72f834298b73898
```

Rectangle horizontal edges are 113.4 pt (40.005 mm); vertical extent is
13.38 pt (4.720 mm); tick interval is 28.35 pt (10.00125 mm). The label
text `40mm 30dot` is nominal: 13.38 pt is 29.733 pixels at 160 dpi, not an
exact 30-pixel center separation. Pixel occupancy after threshold is 30 rows.
No `/Rotate`, `/CropBox`, `/UserUnit`, or `cm` exists in the source PDFs.
Only MediaBox width changes, 141.732283 pt to 180 pt.

Direct raster payload 315 is exactly the first 1260 bytes of direct raster
400; the extra 340 bytes (85 columns) are zero. Both preserve identical
artwork and its placement. Thus changing the source page alone did not
scale the drawing.

### 2. CUPS filter: demonstrated cause

The queue's PPD was retrieved through CUPS' supported read-only PPD export,
not by elevating access to `/etc/cups` or the spool. Frozen copy:
`geometry-2026-10-21/queue.ppd`. It declares a 34.015748 x 141.732283 pt
borderless choice (12 x 50 mm), custom-size parameters, 160 dpi, and PDF
pass-through after the PDF filter. It does not itself contain the source
artwork.

The offline filter chain is `pdftopdf`. With that PPD and the original job
options, expanded output PDFs contain these content matrices:

```text
315 source, media=12x50mm.Borderless:
0 -1 1 0 -0 141.732284 cm

400 source, media=custom_12x63.5mm:
0 -0.787402 0.787402 0 3.615847 141.732283 cm
```

Output MediaBox is 34.015748 x 141.732283 pt in both cases. The scale
magnitude is 141.732283/180 = **0.787401572**, approximately 315/400;
the rounded 315-column count is not the exact PDF scale numerator.
The translation centers the shrunken 12-mm cross dimension in the same
output page. The matrix includes a quarter turn, not an axis-specific shrink.

The installed libppd probe (`ppd-probe.c`) finds **no marked PageSize and
no selected size** for `media=custom_12x63.5mm`. The named borderless
option selects 12 x 50 mm. Thus recording the requested `media` string on
CUPS job 48 did not establish that the custom size had been resolved.
Filter debug records show content height 180 pt versus selected output
height 141.732 pt. This is the concrete size mismatch that precedes fitting.

Controlled toggle: add only `print-scaling=none` to the same malformed-media
offline call. The filtered PDF retains its original 180 x 34.015748 pt page,
and its rendered raster is 400 columns with **the exact direct-400 raster
hash**. The symptom appears/disappears without changing the encoder,
transport, battery, cassette, or firmware.

Additional caution: `media=Custom.12x63.5mm` selects a custom size in a
libppd option probe, but **alone** the installed PDF filter produced an A4
page in this offline control. That input was rejected by the application's
12-mm PDF check. With `print-scaling=none` it retains the original page.
Consequently, merely changing the case/punctuation of the media name is
not a verified end-to-end fix. No queue option was changed.

The intended width-only experiment accidentally changed multiple variables:
PDF page length (50 to 63.5 mm), CUPS media selection (resolved named size
to an unresolved custom keyword), and consequently the filter's geometry
transform. Battery charging and cassette reseating were additional physical
preparations. The resulting hardware feed-count comparison was never a
controlled 315-versus400 test.

### 3. Application render and packing: no observed shrink

`document.rs:127-133` runs Poppler at a single 160 dpi; `178-196` performs
portrait orientation by coordinate permutation and centered-band extraction,
not resampling. `native/adapter.c:59-68` passes the supplied PDF filename
to Rust. PAPPL's default scaling none cannot undo matrices embedded by CUPS.

Row-major pixels become four bytes per feed column. Within each column,
the bytes run bottom-group first and the bits within a byte are MSB-first.
The independent decoder reverses each four-byte group then expands bits;
it agrees with pinned S1 Canvas and S2 encodeBitmap, rather than importing
the project's inverse method. S1's author-facing 30 rows add one row;
our full-frame coordinates already include that padding. Modern mode byte
`81` is not a literal 129-bits-per-pixel claim; legacy byte is `01`.

No width-dependent path exists in `Raster::pack_columns` (`protocol/src/lib.rs:109-119`).
Width/height fields are little-endian u32, width is feed columns, height
32. There is no row stride rounded from 315 to a byte boundary: every
column is exactly four bytes. No transposition, width/height swap, or
implicit resize was found in packing. Both calibration rasters have white
frame rows 0 and 31. The shrunken one has additional white rows 1-3/28-30;
this is already present before packing.

The PNG path is a separate fit-to-box renderer (`letratag-render/src/lib.rs:198-229`).
For the historical 1000 x 80 input, 400-column/30-active-row settings fit
to 375 x 30 inside the 400 x 32 frame. The reconstructed complete mock
capture is byte-identical to the retained Oct 20 mock, despite a later
input-file timestamp. This validates that proxy, not the missing hardware bytes.

### 4. Recorded IPP -> PAPPL -> legacy-render boundary

A new loopback-only recorder forwarded the original and filtered documents
to an explicitly `--transport mock --profile legacy --capture` PAPPL
instance. No hardware address was configured. Raw IPP requests, extracted
document bytes, parsed attributes, job states, and legacy captures are
preserved in `geometry-2026-10-21/boundary/`. Both processes were stopped
afterward. The recorder does not alter the IPP body/document.

| Control | IPP document/format actually received | Explicit media-col (1/100 mm) | Final state and raster |
|---|---|---|---|
| direct315, job 1 | Original315, byte-identical SHA; `application/pdf` | 1200 x 5000 | completed, 315 x 32, edge252/rows30, direct315 raster hash |
| direct400, job 2 | Original400, byte-identical SHA; `application/pdf` | 1200 x 6350 | completed, 400 x 32, edge252/rows30, direct400 raster hash |
| filtered315, job 3 | Offline CUPS315 PDF, byte-identical to filter output; `application/pdf` | 1200 x 5000 | completed, 315 x 32, edge252/rows30, CUPS315 raster hash |
| filtered400, job 4 | Offline CUPS400 PDF, byte-identical to filter output; `application/pdf` | 1200 x 5000 | completed, 315 x 32, edge199/rows24, CUPS400 raster hash |
| none400, job 5 | CUPS400 scaling-none PDF; `application/pdf` | 1200 x 6350 | completed, 400 x 32, edge252/rows30, direct400 raster hash |
| none400-wrong-media, job 6 | Exact same document as job 5 | **1200 x 5000** | **aborted**, no raster capture/transport call |

All these requests explicitly set print-scaling none, zero margins, and
omit orientation-requested, so PAPPL uses its orientation default none.
The IPP collection values above are actual captured request bytes, not
guesses from a media keyword. Final completed/aborted states were retrieved
separately; initial successful Print-Job acceptance only means queued.

PAPPL's `job-ipp.c:140-199` spools received document bytes and submits the
file. `job-process.c:164-170` imports media-col over its default media,
and `530-538` invokes the PDF callback. Native callback passes the spool
filename and interpreted dimensions to Rust. Rust checks width1200,
allowed media length, scaling none, and rendered feed width within one
pixel of `media_length * 160 / 2540` (`ffi.rs:197-224`). The wrong-media
control demonstrates rejection rather than a hidden fit to media.

Before centered-band extraction, Poppler at 160 dpi renders the filtered
315 and fitted400 PDFs to **76 x 315** full-page images. The scaling-none
PDF renders to **400 x 76**. After the application's quarter-turn mapping,
the corresponding feed/frame dimensions above result. No additional
geometry shrink appears at the IPP/PAPPL boundary or legacy encoding in
these controls. In particular, same-source 315 direct versus filtered315
has equal 252/30 feature sizes, despite a threshold-sensitive one-pixel
position difference. This does not account for a factor-two historical
feed difference.

This control is **not a fresh full scheduler-to-PAPPL run**. An attempt to
start an isolated scheduler was denied by executable permissions. Creating
a separate mock-only queue on the existing scheduler returned
`lpadmin: Unauthorized`; read-back confirmed no queue existed. No bypass
or redirect of the hardware queue was attempted. Therefore the historical
actual CUPS-forwarded media-col, orientation and scaling attributes remain
unknown. Offline `cupsfilter` plus recorded IPP replay isolates the geometry
mechanism while leaving that scheduler-attribute gap explicit. Reproducing
the entire live scheduler leg would require authorized mock-queue setup,
not a physical print.

The original direct-IPP **hardware** acceptance had no physical dimensional
measurements. The approximately160-dpi historical labels were PNG sends,
not a dimensionally measured same-PDF direct-IPP comparison. Introducing
CUPS is an important architectural boundary, but the retained hardware
experiments do not isolate it as the sole cause of the older pitch conflict.

## Raster and byte comparison

All rows below are **observed local/mock artifacts**, not hardware packets.
The horizontal stroke is measured separately from tick/square protrusions.

| Artifact | ESC D bytes | Raster/body bytes | Ink bbox inclusive | Horizontal edge / occupied rows |
|---|---|---|---|---|
| Oct 20 HELLO mock | `1b 44 81 02 90 01 00 00 20 00 00 00` | 1600 / 1628 | x124..275, y8..23 | no rectangle; 152 x 16 ink occupancy |
| Direct315 | `1b 44 81 02 3b 01 00 00 20 00 00 00` | 1260 / 1288 | x10..262, y1..30 | 252 / 30 |
| Direct400 | `1b 44 81 02 90 01 00 00 20 00 00 00` | 1600 / 1628 | x10..262, y1..30 | 252 / 30 |
| Reconstructed CUPS315 | `1b 44 81 02 3b 01 00 00 20 00 00 00` | 1260 / 1288 | x9..261, y1..30 | 252 / 30 |
| Preserved CUPS400 reproduction | same as Direct315 | 1260 / 1288 | x7..206, y4..27 | 199 / 24 |
| New offline CUPS400 reconstruction | same as Direct315 | 1260 / 1288 | x7..206, y4..27 | 199 / 24 |
| Offline CUPS400, scaling none | same as Direct400 | 1600 / 1628 | x10..262, y1..30 | 252 / 30 |

The direct bbox has 253 occupied columns because an endpoint marker
protrudes one pixel beyond the 252-pixel horizontal stroke. CUPS400's bbox
is 200 columns, horizontal stroke 199. Across occupied sizes 30/24 have
center spans 29/23; edge-to-edge measurements should not be divided by
center spans without specifying that convention.

Full raster SHA-256:

```text
HELLO:          76365e366e47c7aa0f9bbfe49c905125c7306c3a8ed9567da6c39cfba9681927
Direct315:      8bc8c67134917674443daa1fe5e4120c3fcbf707dd4775395ef6ce5b9710999d
Direct400/none: 781cd94c7e973efdc29b6cfa2200e604a729cc02712d62abfbe5d4031ab8bccf
CUPS315:        ccd7f33e6e04192bd1f9ee04fb0143bcc6ef4fcca28ad72627738be3ba7f9441
CUPS400, both:  4e05e82fe81446da29ab832621b3ce82fce5f5eab210d95f9e05a54cfa6e253e
```

The local capture header includes an additive checksum over its first
eight bytes, **not a raster-content checksum**. The 1288-byte modern bodies
have header `ff f0 12 34 08 05 00 00 42`; direct400's 1628-byte body has
`ff f0 12 34 5c 06 00 00 97`. Both CUPS400 runs agree byte-for-byte on
raster and modern body, not merely on dimensions.

Hardware legacy model: no `ESC # 01`, `ESC D 01 02`, then raster, then
`ESC E ESC A ESC Q`. Body length = 24 + 4W. Logs for jobs 44/46/47/48 imply
W=315: 1284 body bytes and `[9,242,242,242,242,242,82]` writes. The expected
header is `ff f0 12 34 04 05 00 00 3e`, height 32/alignment 2 from source,
but these field/header bytes were **not captured on hardware**. A real
400-column legacy job would have body 1624, header
`ff f0 12 34 58 06 00 00 93`, and `[9,242 x6,181]`. Oct 20 logs match
that length model. Payload SHA-256 for individual hardware jobs is unknown.

The visual reconstruction in `raster-comparison.png` is generated from
independently decoded captures. PBMs and raw unframed bodies are retained
under `decoded/` for direct inspection.

## Mathematical reconciliation

These calculations use the reconstructed post-filter pixel dimensions,
not nominal PDF millimeters. Applying them to hardware remains conditional
on the historical hardware artwork matching the reproduction.

| Quantity | Previous calibration | Final calibration |
|---|---|---|
| Post-filter horizontal stroke | 252 pixels | 199 pixels |
| Post-filter occupied across extent | 30 rows | 24 rows |
| Latest physical rectangle | 20.25 x 4.65 mm | 16.25 x 3.65 mm |
| Effective feed mm/pixel | 0.08036 | 0.08166 |
| Effective feed dpi | 316.09 | 311.05 |
| Effective across mm/pixel | 0.15500 | 0.15208 |
| Effective across dpi | 163.87 | 167.01 |

If end-marker bbox widths 253/200 were measured instead of stroke lengths,
feed results are 317.34/312.62 dpi. These differences illustrate edge
definition and quantization, not demonstrated changes of device resolution.
The initial across reading 4.75 mm instead of the latest 4.65 mm would give
160.42 dpi; it is recorded separately, not substituted to improve agreement.

Measured ratios: feed 16.25/20.25 = **0.80247**; across 3.65/4.65 =
**0.78495**. Raster ratios: 199/252 = **0.78968**, 24/30 = **0.80000**.
Using the previous measured pitch predicts 15.99 x 3.72 mm for the final
rectangle, versus 16.25 x 3.65 mm reported. Differences are 0.26/0.07 mm;
do not invent measurement error bars to force exact agreement. The known
0.787402 PDF shrink and integer thresholded edges explain the approximate
two-axis relationship without a new stepping mode.

At an isotropic 160 dpi, post-filter strokes would be 40.005/31.592 mm
along feed and 4.7625/3.810 mm across. At 200 dpi they would be
32.004/25.273 mm and 3.810/3.048 mm. Neither isotropic model describes all
latest measurements. This is not sufficient to prescribe a new resolution.

Earlier HELLO proxy: 152 occupied columns / 24.30 mm gives **158.88 dpi**;
16 occupied rows / 2.25-2.50 mm gives **180.62-162.56 dpi**. Those are
coarse/corrected operator measurements of text, not a precision native-head
calibration. Compared to today's conditional feed values, a discrepancy
remains. Missing hardware raster bytes and measurement semantics prevent
identifying a state transition as its cause.

### Constant cut-to-cut length

The software output frame is 315 columns in both calibration cases. CUPS
keeps the **same 50-mm page** and moves/scales the content within it. The
BLE body contains width/height/raster/feed/end commands, **no 50 or 63.5-mm
media length field**. The host media setting affects the wire through the
PDF and column count; the printer does not receive the CUPS media keyword.

Thus a changed firmware minimum-length rule is unnecessary to explain
equal cut lengths. A firmware minimum or cutter offset could still exist;
these jobs cannot distinguish that because their derived wire widths are
equal. Across shrinking and increased trailing whitespace are encoded
before BLE. Physical rectangle loss 4.00 mm versus trail increase 3.75 mm
is broadly consistent with redistribution inside a fixed frame, not exact
enough to derive an additional mechanism. Latest lead/total equality does
not supply new absolute lengths. Earlier lead 10 + rectangle 20.25 + latest
trail 13.25 = 43.50 mm differs from the earlier total 43.15 mm; retain the
0.35-mm inconsistency instead of silently treating separate readings as exact.

## Confirmed defects and surviving hypotheses

| Finding/hypothesis | Support and contradiction | Falsifier / smallest distinguishing offline test | Confidence |
|---|---|---|---|
| CUPS fits the final artwork to a different page | Explicit matrix; exact raster hash match; scaling-none toggle removes it. No contrary retained software evidence | A recovered historical filtered job-48 PDF with no shrink would weaken historical attribution, while leaving the reproduced defect real | High for local cause; high but not packet-proven for hardware job |
| Preflight verified the wrong pipeline stage | Only direct source raster was verified; hardware lengths disagree with 400. CUPS reproduction explains mismatch | Post-filter width/landmark guard, already exercised offline | High, confirmed methodology bug |
| Width/height swap, stride or packing resize | Byte decode/source/pinned external layouts agree; shrinking is already in filtered PDF | Hand-authored asymmetric pixel fixture and independent decoded PBM | No supporting evidence for this calibration shrink |
| BLE corruption/chunking causes shrink | No raw hardware trace, so all device-side errors cannot be excluded. Contradicted as the software-shrink cause by BLE-free reproduction | Compare retained post-filter raster to a future authorized packet trace; no physical test requested | Low as shrink cause |
| Variable feed stepping caused this final change | Both derived hardware widths are 315 and both artwork axes shrink upstream; no controlled 315-versus400 hardware comparison exists | Existing PDF toggle already distinguishes upstream shrink; this print cannot test width thresholds | Unsupported by this experiment |
| Persistent state/mechanics/power explains older feed discrepancy | Historical physical ratios differ. Inputs, rendering path, session, measurement method also differ. Battery substitution only reproduced one current setup | First recover historical transmitted raster or ruler/photo reference with unambiguous landmarks; inspect provenance before proposing hardware tests | Open, low confidence in any specific mechanism |
| About 160 dpi across / about 310 dpi apparent feed today | Conditional post-filter counts and measurements fit this description. Historical feed proxy gives about 159 dpi | Recover actual job bytes and reconcile measurement definitions; universal native pitch remains unverified | Moderate as a description, insufficient as a driver model |
| Fixed firmware minimum cut length | Equal total lengths compatible, but host sent equal frame widths already | Existing data cannot separate a firmware floor from equal-width jobs | Unknown; not needed to explain observed equality |

## Minimal justified correction and regression coverage

Do **not** change layout DPI, advertise anisotropic resolution, stretch
columns, or tune BLE. The demonstrated correction belongs to job-option
validation and **post-CUPS preflight**: resolve the actual media size, require
unscaled geometry, and reject an unsupported/substituted size before any
hardware send. Explicit scaling none removes this filter's shrink offline,
but does not by itself prove that the forwarded IPP media-col is 1200 x 6350.
The current media validation could correctly reject that unresolved mismatch.
No production fix is implemented in this investigation.

Tests added, all offline:

1. `PYTHONDONTWRITEBYTECODE=1 python3 scripts/test-forensic-raster.py`:
   hand-built asymmetric wire fixture, framing/checksum/chunk errors, source
   stream equality, exact raster-prefix equality, and known shrunken capture.
2. `bash scripts/test-cups-geometry.sh`: frozen PPD and installed `pdftopdf`,
   source315 control, historical failing custom400 options, geometry guard
   rejecting 315 when 400 is required, scaling-none control retaining
   400 and unchanged artwork. Fresh output raster hashes must match direct
   controls and preserved fixtures; the failing case must have edge199/rows24.
   It runs `mock`, never `lp`, a backend, or BLE. The optional
   `LETRATAG_FORENSIC_TMPDIR` selects an existing scratch parent when
   `/tmp/opencode` is unavailable in an execution sandbox.

This is a reproducer/control suite, not a claim that production CUPS
submission is fixed. Further software-only acceptance should exercise
exact forwarded IPP media dimensions, both source orientations, custom
lengths, unsupported-size rejection, and post-filter landmark equality
against a mock-only destination. No physical test is authorized or proposed.

### Independent review and verification outcome

Read-only review found that the initial shell regression checked only
width/bounds, not complete artwork. It now compares newly generated raster
hashes to a fresh direct control and preserved fixtures, and checks the
199-pixel edge/24-row failing geometry. The README and hardware runbook
were corrected to distinguish adapter defaults from upstream fitting.
Targeted independent review of those changes returned no important findings.

The executing verifier passed all five decoder tests and independently
validated **80/80 manifest entries**, **12 capture decodes**, header
checksums, legacy fields, PDF content equality and scale matrices. Its own
extra edge-start assertion initially failed because it assumed x9 rather
than the actual horizontal-edge x10; targeted PBM inspection resolved that
without changing evidence or production code. It also verified no tracked
production/version changes.

The **final automated CUPS regression script remains execution-blocked in
the verifier sandbox**, not passed. The first attempt lacked
`/tmp/opencode`; the deliberately scoped scratch-parent control then failed:

```text
mktemp: failed to create directory via template '/work/test-output/letratag-geometry-check.XXXXXXXX': Read-only file system
```

No fallback or further gate replay followed that sandbox denial. The
author's individual offline filter/mock and IPP controls did execute and
their saved results were independently decoded/hashed. That is distinct
from the final shell regression being independently executed. No new Rust
build/test gate is claimed for this documentation/forensics-only change.

### Remaining uncertainties

Missing actual hardware payloads, historical forwarded IPP media-col and
filtered documents, unambiguous physical edge conventions, corrected old
caliper provenance, and detailed power/cassette history limit conclusions.
The new software scaling mechanism is demonstrated. The older apparent
feed-pitch discrepancy is an **open hardware/evidence finding**, not a
proven firmware mode. Existing geometry/protocol comments asserting native
160 dpi more strongly are left unchanged, not validated by this report.

### Reproduction environment and source checks

Repository baseline `910ee70`; existing debug binary predates this review.
Installed: CUPS/libcups 2.4.20; cups-filters 2.0.1; libppd 2.1.1;
libcupsfilters 2.2.1.r32.gd49e9284; Poppler 26.08.0; qpdf 12.4.2.
The PPD is the current CUPS export, not an immutable historical PPD snapshot.
The `cupsfilter -d` attempt unexpectedly used a missing laserjet PPD and
failed; successful controls explicitly use `-p` with the exported PPD.
No successful run relies on that failed attempt.

Pinned wire-layout comparisons were read from:

- S1 `ysfchn/dymo-bluetooth`, commit `1c1b30a6612de6bf880d5f077a23630b83810750`,
  `dymo_bluetooth/printer.py`, Canvas/DirectiveBuilder/command_print.
- S2 `thermal-label/letratag`, commit `a8fb41e19ce3a8cb1e51d6aaeb1acc8060d5842b`,
  `packages/core/src/protocol.ts`, encodeBitmap/buildPrintData/chunkPayload.

These establish compatibility of the decoded layout, not an independently
verified native printer pitch or a guarantee about firmware behavior.
