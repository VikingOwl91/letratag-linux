#!/usr/bin/env bash
# Offline filter and mock commands only. Never submit to a CUPS queue.
#
# Controls the demonstrated CUPS pdftopdf media-fitting defect at the
# current calibration model: a 63.5 mm PDF submitted with an unresolved
# custom-media keyword is fitted into the 50 mm page (0.787402 scale on
# both axes), so the raster width violates the geometry guard; adding
# print-scaling=none preserves the intended columns and artwork. The
# committed captures under docs/evidence/geometry-2026-10-21/ remain as
# historical evidence of the 160-dpi model and are not modified.
set -euo pipefail
root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root"
evidence=docs/evidence/geometry-2026-10-21
lab=$(mktemp -d "${LETRATAG_FORENSIC_TMPDIR:-/tmp/opencode}/letratag-geometry-check.XXXXXXXX")
echo "Evidence: $lab"
binary=${LETRATAG_BINARY:-target/debug/letratag-printer}
# Intended physical widths at the default calibration (315 DPI feed):
# 50 mm -> 620 columns, 63.5 mm -> 788 columns.
"$binary" mock "$evidence/calibration-400.pdf" --pdf --state "$lab/direct788-state" --capture
python3 scripts/forensic-raster.py "$lab/direct788-state/"*.ble > "$lab/direct788.json"
for variant in original620 invalid788 none788; do
    input="$evidence/calibration-400.pdf"
    options=(-o media=custom_12x63.5mm)
    if [[ $variant == original620 ]]; then
        input="$evidence/calibration-315.pdf"
        options=(-o media=12x50mm.Borderless)
    elif [[ $variant == none788 ]]; then
        options+=(-o print-scaling=none)
    fi
    cupsfilter -p "$evidence/queue.ppd" -i application/pdf \
        -m application/vnd.cups-pdf "${options[@]}" "$input" \
        > "$lab/$variant.pdf" 2> "$lab/$variant.log"
    "$binary" mock "$lab/$variant.pdf" --pdf --state "$lab/$variant-state" --capture
done
python3 scripts/forensic-raster.py "$lab/original620-state/"*.ble \
    --expect-width 620 --expect-ink-size 498 30 > "$lab/original620.json"
if python3 scripts/forensic-raster.py "$lab/invalid788-state/"*.ble \
    --expect-width 788 --expect-ink-size 498 30 > "$lab/invalid788.json" 2> "$lab/guard.log"; then
    echo "FAIL: expected the historical custom-media submission to violate the geometry guard"
    exit 1
fi
grep -q 'expected width 788, got 620' "$lab/guard.log"
python3 scripts/forensic-raster.py "$lab/invalid788-state/"*.ble > "$lab/invalid788.json"
python3 scripts/forensic-raster.py "$lab/none788-state/"*.ble \
    --expect-width 788 --expect-ink-size 498 30 > "$lab/none788.json"
python3 - "$lab" <<'PY'
import json
from pathlib import Path
import sys

lab = Path(sys.argv[1])
def one(name):
    return next(iter(json.loads((lab / (name + '.json')).read_text()).values()))
direct = one('direct788')
good = one('none788')
bad = one('invalid788')
original = one('original620')
# The scaling-none control preserves the intended 788 columns and the exact
# artwork of the direct render.
assert good['raster_sha256'] == direct['raster_sha256']
# The 50 mm control carries the identical artwork at the identical columns;
# the longer page adds blank columns only.
assert original['ink_bbox_inclusive'] == direct['ink_bbox_inclusive'] == [19, 1, 516, 30]
assert direct['ink_bbox_inclusive'][2] < 620
# The custom-media submission was fitted to 50 mm: 620 columns, artwork
# shrunk by 0.787402 on both axes (498x30 -> 392x24).
assert bad['width'] == 620
assert bad['ink_occupied_size'] == [392, 24]
assert bad['ink_count_per_row'][4] == bad['ink_count_per_row'][27] == 390
assert original['ink_count_per_row'][1] == original['ink_count_per_row'][30] == 496
PY
echo 'PASS: reproduced CUPS shrink, rejected it with the guard, preserved 788 columns with scaling none'
