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

# `git archive` writes a pax global header that carries the commit id, and
# stamps every entry with the commit time. Both would make the archive bytes
# depend on the commit rather than on its content, which would break the
# packaging checksum that pins this archive. Drop the header and normalize
# the timestamps, so the archive describes the file tree only.
python3 - "$archive" <<'PY'
import sys

path = sys.argv[1]
data = bytearray(open(path, "rb").read())

pos = 0
out = bytearray()
while pos < len(data):
    header = data[pos:pos + 512]
    if len(header) < 512:
        break
    name = header[0:100].rstrip(b"\0")
    typeflag = header[156:157]
    size = int(header[124:136].rstrip(b"\0 ").decode() or "0", 8)
    if name == b"pax_global_header" and typeflag == b"g":
        pos += 512 + ((size + 511) // 512) * 512
        continue
    if name:
        header[136:148] = b"00000000000\x00"
        # The ustar header checksum covers the whole header, so it has to be
        # recomputed after the timestamp changed.
        header[148:156] = b" " * 8
        checksum = sum(header)
        header[148:156] = ("%06o\x00 " % checksum).encode("ascii")
    out += header
    out += data[pos + 512:pos + 512 + ((size + 511) // 512) * 512]
    pos += 512 + ((size + 511) // 512) * 512
    if not name:
        # End of archive: keep the trailing zero blocks verbatim.
        out += data[pos:]
        break

open(path, "wb").write(bytes(out))
PY

echo "commit:  $commit"
echo "archive: $archive"
echo "sha256:  $(sha256sum "$archive" | cut -d' ' -f1)"
echo "size:    $(stat -c%s "$archive") bytes"
