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

GitHub's private vulnerability reporting is **not enabled on this
repository yet**. It is a single setting, and any repository maintainer can
turn it on:

1. Open **Settings → Security and analysis → Security advisories**.
2. Under **Report a vulnerability**, choose **Enable private vulnerability
   reporting**.

Once that is done, the **Security** tab of the repository shows a
**Report a vulnerability** link, and reports arrive as a private GitHub
advisory that only the reporter and the maintainers can see.

Until it is enabled, please start a private conversation through the
maintainer's GitHub profile and ask for a private channel before sharing
any technical detail. If that is not possible, open an issue that
contains **no** exploit details, only a request to establish a private
channel, and wait for a reply.

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
