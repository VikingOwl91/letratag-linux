#!/usr/bin/env bash
# Offline filter and mock commands only. Never submit to a CUPS queue.
set -euo pipefail
root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root"
evidence=docs/evidence/geometry-2026-10-21
lab=$(mktemp -d "${LETRATAG_FORENSIC_TMPDIR:-/tmp/opencode}/letratag-geometry-check.XXXXXXXX")
echo "Evidence: $lab"
binary=${LETRATAG_BINARY:-target/debug/letratag-printer}
"$binary" mock "$evidence/calibration-400.pdf" --pdf --state "$lab/direct400-state" --capture
python3 scripts/forensic-raster.py "$lab/direct400-state/"*.ble > "$lab/direct400.json"
for variant in original315 invalid400 none400; do
    input="$evidence/calibration-400.pdf"
    options=(-o media=custom_12x63.5mm)
    if [[ $variant == original315 ]]; then
        input="$evidence/calibration-315.pdf"
        options=(-o media=12x50mm.Borderless)
    elif [[ $variant == none400 ]]; then
        options+=(-o print-scaling=none)
    fi
    cupsfilter -p "$evidence/queue.ppd" -i application/pdf \
        -m application/vnd.cups-pdf "${options[@]}" "$input" \
        > "$lab/$variant.pdf" 2> "$lab/$variant.log"
    "$binary" mock "$lab/$variant.pdf" --pdf --state "$lab/$variant-state" --capture
done
python3 scripts/forensic-raster.py "$lab/original315-state/"*.ble \
    --expect-width 315 --expect-ink-size 253 30 > "$lab/original315.json"
if python3 scripts/forensic-raster.py "$lab/invalid400-state/"*.ble \
    --expect-width 400 --expect-ink-size 253 30 > "$lab/invalid400.json" 2> "$lab/guard.log"; then
    echo "FAIL: expected the historical custom-media submission to violate the geometry guard"
    exit 1
fi
grep -q 'expected width 400, got 315' "$lab/guard.log"
python3 scripts/forensic-raster.py "$lab/invalid400-state/"*.ble > "$lab/invalid400.json"
python3 scripts/forensic-raster.py "$lab/none400-state/"*.ble \
    --expect-width 400 --expect-ink-size 253 30 > "$lab/none400.json"
python3 - "$lab" "$evidence" <<'PY'
import json
from pathlib import Path
import sys

lab, evidence = map(Path, sys.argv[1:])
def one(name):
    return next(iter(json.loads((lab / (name + '.json')).read_text()).values()))
saved = json.loads((evidence / 'decoded.json').read_text())
by_name = {Path(name).name: result for name, result in saved.items()}
direct = one('direct400')
good = one('none400')
bad = one('invalid400')
original = one('original315')
assert direct['raster_sha256'] == by_name['direct-400.ble']['raster_sha256']
assert good['raster_sha256'] == direct['raster_sha256']
assert original['raster_sha256'] == by_name['filtered-315.ble']['raster_sha256']
assert bad['raster_sha256'] == by_name['cups-400-reproduction.ble']['raster_sha256']
assert bad['ink_occupied_size'] == [200, 24]
assert bad['ink_count_per_row'][4] == bad['ink_count_per_row'][27] == 199
assert original['ink_count_per_row'][1] == original['ink_count_per_row'][30] == 252
PY
echo 'PASS: reproduced CUPS shrink, rejected it with the guard, preserved 400 columns with scaling none'
