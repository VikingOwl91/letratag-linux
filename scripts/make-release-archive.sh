#!/usr/bin/env bash
# Build the reproducible release source archive for a release tag.
#
# The archive is produced with `git archive`, so its content is exactly the
# tree of the given commit and nothing else: no untracked files, no build
# output, no credentials. The uncompressed tar form is used on purpose, the
# gzip wrapper is not byte-reproducible across zlib versions.
#
# `packaging/arch/` is excluded from the archive. The Arch recipe carries
# the SHA256 of this archive, so including it would make the checksum depend
# on itself. Everything else, including the systemd unit, is included
# verbatim.
#
# Usage: scripts/make-release-archive.sh <version> [ref] [output-directory]
#
#   scripts/make-release-archive.sh 0.3.0
#   scripts/make-release-archive.sh 0.3.0 v0.3.0
#
# Prints the archive path and its SHA256. The same command must produce the
# file that is uploaded as the release asset, because the packaging
# checksum in packaging/arch/PKGBUILD is the checksum of exactly this file.
set -euo pipefail
root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
cd "$root"

if [[ $# -lt 1 || $# -gt 3 ]]; then
  echo "usage: $0 <version> [ref] [output-directory]" >&2
  exit 2
fi

version=$1
ref=${2:-HEAD}
outdir=${3:-"$root/test-output/release"}

if [[ ! "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  echo "version must look like 0.3.0, got: $version" >&2
  exit 2
fi

commit=$(git rev-parse --verify "$ref^{commit}")
name="letratag-linux-$version"
mkdir -p "$outdir"
archive="$outdir/$name.tar"

# Refuse to package a dirty tree: the archive must describe one commit.
if [[ "$ref" == "HEAD" ]] && ! git diff --quiet HEAD --; then
  echo "working tree has uncommitted changes; commit them before archiving" >&2
  exit 1
fi

rm -f "$archive"
git archive --format=tar --prefix="$name/" "$commit" -- . ':(exclude)packaging/arch' > "$archive"

# `git archive` writes a pax global header that carries the commit id, which
# would make the archive bytes depend on the commit rather than on its
# content. Drop that header so the archive describes the file tree only.
python3 - "$archive" <<'PY'
import sys

path = sys.argv[1]
data = open(path, "rb").read()

if data[0:100].rstrip(b"\0") == b"pax_global_header" and data[156:157] == b"g":
    size = int(data[124:136].rstrip(b"\0 ").decode() or "0", 8)
    offset = 512 + ((size + 511) // 512) * 512
    open(path, "wb").write(data[offset:])
PY

echo "commit:  $commit"
echo "archive: $archive"
echo "sha256:  $(sha256sum "$archive" | cut -d' ' -f1)"
echo "size:    $(stat -c%s "$archive") bytes"
