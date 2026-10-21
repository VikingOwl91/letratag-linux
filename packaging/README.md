# Packaging preparation

No system package or service has been installed by this project.

[Direct IPP](../docs/direct-ipp.md) is the officially recommended/default
workflow. The application provides its own loopback PAPPL IPP endpoint;
it does not require a system CUPS scheduler or a registered queue.
[CUPS queue integration](../docs/cups-experimental.md) is Experimental.
Both workflows use the same server and Rust/BlueZ implementation, with mock
transport unchanged as the default. Graphical application support for Direct
IPP varies and is not a packaging guarantee.

PAPPL and libcups remain native dependencies. The Arch recipe lists
`libcups`, not the CUPS scheduler package. Command-line clients such as `lp`
are useful for Direct IPP but their presence does not require starting a
scheduler. Check the target distribution's client package split separately;
this recipe does not establish which package supplies each client tool.

`arch/PKGBUILD` is a complete, locally validated recipe for v0.3.0, not an
AUR submission. It was verified on Arch Linux with `makepkg`:
`makepkg --verifysource` passes for both sources, and a full build produces
`letratag-linux-0.3.0-1-x86_64.pkg.tar.zst` after the recipe's own `check()`
phase runs the complete test suite.

* The source is the release asset `letratag-linux-0.3.0.tar`, produced by
  `scripts/make-release-archive.sh 0.3.0` from the release commit. The
  archive is content-only: it excludes `packaging/arch/` (the recipe
  references the archive by checksum) and carries no commit id or
  timestamps, so the checksum in the recipe stays valid.
* The recipe pins the upstream PAPPL 1.4.9 release tarball by SHA256 and
  applies `patches/pappl-1.4.9-document-policy.patch` to it, then builds
  PAPPL into a private prefix inside the build directory. PAPPL is linked
  statically, so the installed package has no runtime PAPPL dependency.
  Unpatched distribution PAPPL is not sufficient for the PDF-first policy.
  Do not replace the system PAPPL library as a side effect of building this
  application.
* The package installs the binary, the mock-only systemd unit, the project
  license and the PAPPL license and notice.

The recipe can only be built after the release asset exists, because the
asset is the release archive of the tagged commit. The checksum in the
recipe is the checksum of exactly that file; regenerate it and compare
before publishing.

The source build helper already keeps the clean pinned reference checkout,
patched build tree and private installation prefix separate. Its output is
for project-local development only. It records the upstream revision and patch
SHA256 under the private prefix's `share/letratag-build` directory.

`systemd/letratag-linux.service` is a hardened **mock-only** service template.
It uses DynamicUser and private state, no capabilities, no automatic restart,
and loopback-only network policy. Its `ExecStart` assumes a future packaged
binary at `/usr/bin/letratag-printer`; it is not activated during tests.
Actual service startup, D-Bus permission policy, resource limits on a running
systemd instance and application-specific Direct IPP support remain separate
installation acceptance checks requiring explicit authorization. Experimental
CUPS queue registration is optional and requires its own explicit authorization
and mock-only geometry/forwarded-attribute checks.

The server exits its process after PAPPL's run loop rather than assuming that
PAPPL drains detached client threads. It is not an embeddable/reusable server
API. PAPPL's own internal shutdown behavior still needs upstream review and
sanitizer testing. Preserve unresolved print journals after any process exit.
