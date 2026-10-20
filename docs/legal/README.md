# License review

Project source: MIT, see the root `LICENSE`.

The PAPPL derivative patch under `patches/` follows PAPPL's Apache-2.0 terms;
the root MIT grant does not replace the upstream license on patch context.

## Protocol references

| Source | Reviewed revision | License | Reuse policy |
|---|---|---|---|
| ysfchn/dymo-bluetooth | `1c1b30a6612de6bf880d5f077a23630b83810750` | MIT | Interoperability reference; retain copyright and permission notice |
| thermal-label/letratag | `a8fb41e19ce3a8cb1e51d6aaeb1acc8060d5842b` | MIT | Interoperability reference; retain copyright and permission notice |
| alexhorn/lt200b | `129c1f206db8a3b3f13bfae09901e12507a80c53` | AGPL-3.0 | Comparison only; no implementation or documentation copied/ported |

The original MIT texts are retained here. Packet fixtures are independently
calculated from the cited wire layouts; they are not proprietary device
captures or copied application assets. No source is imported from the
HomeOps/Snipe-IT integration. This is a practical provenance review, not a
legal opinion on reverse engineering in every jurisdiction.

## Native components

PAPPL 1.4.9 is Apache-2.0 with the optional exceptions in its `NOTICE`.
The exact upstream LICENSE and NOTICE are retained. The development helper
builds its static library locally, so distributing the resulting executable
also distributes PAPPL code. Preserve those files and the required notices.

libcups, BlueZ/D-Bus, Avahi, OpenSSL, libpng, libjpeg and zlib retain their
system-package licenses. Poppler and qpdf are invoked as separate system executables,
not linked or copied into this source tree. Distribution packages must declare
these dependencies and satisfy their own license obligations.

## Rust dependencies

`cargo-licenses.tsv` records the resolved names, versions, declared licenses
and repositories from `cargo metadata --locked --all-features`, including
build/test and target-conditional packages. Regenerate with:

```sh
bash scripts/check-licenses.sh
```

The reviewed declarations have permissive choices (MIT, Apache-2.0, BSD,
Zlib, Unlicense or 0BSD), with Unicode-3.0 obligations for `unicode-ident`.
Where a dependency offers MIT/Apache/LGPL alternatives, select a permissive
option and preserve the corresponding notices. SPDX strings using legacy
slashes are recorded verbatim, not silently normalized.

This inventory is not a recursive source-level legal audit, a vulnerability
scan or an automatically generated binary notice bundle. Before a public
binary release, generate and review a complete third-party notice bundle
for the actual target/features and verify system-library packaging licenses.
