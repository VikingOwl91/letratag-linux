#!/usr/bin/env bash
set -euo pipefail
cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.."
cargo metadata --locked --all-features --format-version 1 |
  jq -r '.packages | sort_by(.name,.version)[] | [.name,.version,(.license // "MISSING"),(.repository // "-")] | @tsv'
