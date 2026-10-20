#!/usr/bin/env bash
set -euo pipefail
root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
source_dir="$root/.research/pappl"
prefix="$root/.research/pappl-prefix"
commit=ae1889822d872eddd572a43ad0a4a3674fa5a9fa
mkdir -p "$root/.research"
if [[ ! -d "$source_dir/.git" ]]; then
  git clone --depth 1 --branch v1.4.9 https://github.com/michaelrsweet/pappl.git "$source_dir"
fi
[[ $(git -C "$source_dir" rev-parse HEAD) == "$commit" ]] || { echo 'PAPPL source revision mismatch' >&2; exit 1; }
git -C "$source_dir" diff --exit-code --quiet
git -C "$source_dir" diff --cached --exit-code --quiet
patch_file="$root/patches/pappl-1.4.9-document-policy.patch"
build_dir=$(mktemp -d "$root/.research/pappl-build.XXXXXXXX")
git clone --no-hardlinks "$source_dir" "$build_dir"
git -C "$build_dir" apply --check "$patch_file"
git -C "$build_dir" apply "$patch_file"
(
  cd "$build_dir"
  ./configure --prefix="$prefix" --disable-shared --disable-libusb --disable-libpam
  make -j2
  make install
)
mkdir -p "$prefix/share/letratag-build"
printf '%s\n' "$commit" > "$prefix/share/letratag-build/upstream-commit"
sha256sum "$patch_file" > "$prefix/share/letratag-build/patch.sha256"
printf '\nProject-local PAPPL built. Build with:\nPKG_CONFIG_PATH=%q cargo build --locked --workspace --all-features\n' "$prefix/lib/pkgconfig"
