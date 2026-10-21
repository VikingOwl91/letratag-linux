#!/usr/bin/env bash
set -euo pipefail
cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.."

# The published crates carry their own copy of the license text; keep them
# identical to the repository root file.
for crate in letratag-protocol letratag-render letratag-transport; do
  if ! cmp -s LICENSE "crates/$crate/LICENSE"; then
    echo "FAIL: crates/$crate/LICENSE differs from the repository LICENSE" >&2
    exit 1
  fi
done
echo "PASS: published crate license copies match the repository LICENSE"

cargo metadata --locked --all-features --format-version 1 |
  jq -r '.packages | sort_by(.name,.version)[] | [.name,.version,(.license // "MISSING"),(.repository // "-")] | @tsv'
