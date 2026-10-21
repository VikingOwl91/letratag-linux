# Security policy

## Supported versions

| Version | Supported |
|---|---|
| v0.3.0 (current release) | yes |
| `main` | yes, for issues that are not yet released |
| anything older | no |

Only the DYMO LetraTag 200B has been physically validated, and the driver
runs as an unprivileged local service on `127.0.0.1`. Reports that concern
other printer models, other distributions or a multi-user deployment are
welcome, but they are outside the validated scope of this project.

## Reporting a vulnerability

**Do not open a public issue for an exploitable problem.** A public issue
is indexed, cached and shared; once a detail is public, it cannot be
withdrawn.

This repository has **GitHub private vulnerability reporting enabled**. Use
the report form on the repository's Security tab:

<https://github.com/VikingOwl91/letratag-linux/security/advisories/new>

It requires a GitHub account, and it opens a private advisory that only the
reporter and the maintainers can see. The same entry point is the
**Report a vulnerability** button on the repository's **Security** tab.

If you cannot use the form, open a public issue that contains **no** exploit
detail, only a request to establish a private channel, and wait for a reply
before sharing anything technical.

Please include:

* the affected version or commit,
* what an attacker could achieve, and what access they need,
* the steps to reproduce,
* whether the problem is in the IPP service, the PDF pipeline, the BLE
  transport or the journal handling.

## What to expect

There is no response-time commitment. Reports are triaged in the order they
arrive, and the reporter is kept informed either way. A fix, when it
exists, lands on `main` first and is released with the next version.

## Technical threat model

The security model, the resource limits, the loopback binding, the
uncertain-outcome and journal rules, and the conservative completion
policy are documented in [docs/security.md](docs/security.md). Read it
before reporting: many reports are known limitations of an experimental
driver rather than defects.
