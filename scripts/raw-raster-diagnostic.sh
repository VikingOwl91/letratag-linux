#!/usr/bin/env bash
# One-shot RAW raster diagnostic for the LetraTag 200B.
#
# Runs the cargo example crates/letratag-printer/examples/raw-raster-diagnostic.rs,
# which builds a deterministic in-memory 1-bit raster (no PDF/Poppler/PNG/CUPS/
# IPP/PAPPL/scaling), records payload evidence, and can transmit exactly once
# over the existing BlueZ transport through the existing engine/journal path.
#
#   Dry run (all pre-print verification, no Bluetooth):
#     bash scripts/raw-raster-diagnostic.sh --dry-run \
#       --address 90:E5:B1:49:DA:AE --state test-output/raw-diag-dry
#
#   One physical print (requires explicit acknowledgment; never retries):
#     bash scripts/raw-raster-diagnostic.sh --acknowledge-unverified-hardware \
#       --address 90:E5:B1:49:DA:AE --state test-output/raw-diag-state
set -euo pipefail
umask 077
root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
cd "$root"
export CARGO_NET_OFFLINE=true
export PKG_CONFIG_PATH="$root/.research/pappl-prefix/lib/pkgconfig${PKG_CONFIG_PATH:+:$PKG_CONFIG_PATH}"
state=""
previous=""
for arg in "$@"; do
  if [[ $previous == "--state" ]]; then state=$arg; fi
  previous=$arg
done
if [[ -n $state && $state == "$root"/* ]]; then
  mkdir -p "$state"
  chmod 700 "$state"
  cargo --config .research/vendor-config.toml run --locked -p letratag-printer \
    --features pappl,ble --example raw-raster-diagnostic -- "$@" 2>&1 | tee "$state/run.log"
else
  cargo --config .research/vendor-config.toml run --locked -p letratag-printer \
    --features pappl,ble --example raw-raster-diagnostic -- "$@"
fi
