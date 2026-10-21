# Packaging preparation

No system package or service has been installed by this project.

[Direct IPP](../docs/direct-ipp.md) is the officially recommended/default
workflow. The application provides its own loopback PAPPL IPP endpoint;
it does not require a system CUPS scheduler or a registered queue.
[CUPS queue integration](../docs/cups-experimental.md) is Experimental.
Both workflows use the same server and Rust/BlueZ implementation, with mock
transport unchanged as the default. Graphical application support for Direct
IPP varies and is not a packaging guarantee.

PAPPL and libcups remain native dependencies. The template lists `libcups`,
not the CUPS scheduler package. Command-line clients such as `lp` are useful
for Direct IPP but their presence does not require starting a scheduler.
Check the target distribution's client package split separately; this template
does not establish which package supplies each client tool.

`arch/PKGBUILD.in` is a preparation template, not a ready AUR submission.
Before using it, provide a public project URL, a reviewed source archive,
its real SHA256, and an explicitly maintained patched-PAPPL dependency.
Unpatched distribution PAPPL is not sufficient for the PDF-first policy.
Do not replace the system PAPPL library as a side effect of building this
application. A future package should use a private prefix or a separately
named, versioned library package and regenerate its complete license notices.

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
