# crates.io publication

Status: **published 2026-10-21.** The three library crates are on crates.io
(`letratag-protocol` and `letratag-render` at 0.1.0 and 0.1.1,
`letratag-transport` at 0.1.1; operator-approved, see the publication record
below). `letratag-printer` is deliberately not published.

## Version scheme

* The three library crates carry the workspace version (`0.1.1`) and are
  versioned independently of the application release. `letratag-transport`
  carries `0.1.2` because it gained the btleplug backend after its
  published 0.1.1.
* `letratag-printer` is the application crate. It carries the release
  version (`0.3.0`) so that `letratag-printer --version`, the release tag
  `v0.3.0` and the Arch package `letratag-linux-0.3.0-1` agree.
* Release tags are `vX.Y.Z` of the application release, not of the library
  crates.

Assessment date: 2026-10-21, workspace at `95bf1f0`; publication executed
from `c811dd7` (clean tree, git-ignored `.env` token, no credentials
committed).

**Pending 0.1.2 of `letratag-transport` (prepared 2026-10-21, not
published).** The published 0.1.1 predates the btleplug backend
(`7b7cdd0`), so the crate content no longer matches the published version
and a new version is required for the next publication. The manifest now
carries version 0.1.2 (independent of the workspace version), the
description and README mention the btleplug backend, and `keywords` and
`categories` are set. `letratag-protocol` and `letratag-render` are
unchanged since 0.1.1 and stay at the workspace version, which matches what
is published; the path dependency requirement in the root manifest points at
the published 0.1.1. Publishing 0.1.2 requires the operator's explicit
authorization and a crates.io token.

## Crate-by-crate assessment

| Crate | Published | Versions | crates.io |
|---|---|---|---|
| `letratag-protocol` | yes, 2026-10-21 | 0.1.0, **0.1.1** | <https://crates.io/crates/letratag-protocol> |
| `letratag-render` | yes, 2026-10-21 | 0.1.0, **0.1.1** | <https://crates.io/crates/letratag-render> |
| `letratag-transport` | yes, 2026-10-21 | **0.1.1** (0.1.0 was never published; see correction), 0.1.2 prepared, not published | <https://crates.io/crates/letratag-transport> |
| `letratag-printer` | **no** | — | the `pappl` feature builds against the pinned, patched PAPPL via pkg-config (`scripts/build-pappl.sh`, private prefix); a crates.io build cannot reproduce that, so the published crate would not build its main feature from source |

**Correction 2026-10-21:** an earlier version of this document (and the
verification record) claimed all three library crates were published at
0.1.0. Registry evidence shows that is wrong for `letratag-transport`: the
crates.io API returns 404 "does not exist", the website 404s, the sparse
index entry is absent, and `cargo search` finds nothing. Only
`letratag-protocol` and `letratag-render` are published at 0.1.0 (both
verified via the API and the sparse index). `letratag-transport` was never
uploaded; it will be published fresh at 0.1.1.

`letratag-printer` is an internal application crate: the PAPPL/IPP service,
the CLI, the C adapter and the private patched-PAPPL build all belong to
the repository, not to a registry package. Keep it unpublished.

## Readiness review

* **Names:** `letratag-protocol`, `letratag-render`, `letratag-transport` are
  available on crates.io (verified by the index lookup during the dry runs).
* **License:** MIT for all crates (`license.workspace`). The three published
  crates carry a copy of the license text, checked against the repository
  root file by `scripts/check-licenses.sh` in CI.
* **Metadata:** all four manifests carry `description` and `repository`.
  `documentation`/`homepage` are unset; docs.rs links appear automatically
  after the first publish. The three published crates carry `keywords` and
  `categories`; `letratag-transport` additionally states the btleplug
  backend in its description and README.
* **Dependencies:** `letratag-render` depends on `letratag-protocol`;
  `letratag-transport` is independent of the other two. External
  dependencies are pinned by `Cargo.lock` in the repository;
  `bluer =0.17.4` is pinned exactly (its `mtu_workaround` behavior is load
  bearing). Feature flags: `letratag-transport/ble` (BlueZ via bluer,
  default off), `letratag-printer/{pappl,ble}` (not published).
* **API stability:** 0.1.x. The public APIs are small and tested
  (protocol golden vectors and proptests, transport chunk-sizing and mock
  fault tests, render property tests). The transport API gained
  `ChunkMode`/`TransportParameters`/`parameters()` in 0.1.0 development;
  treat everything as 0.x until the first hardware-backed release.
* **Package contents:** verified with `cargo package --list`; no build
  artifacts, patches, evidence or test-output directories are included.
* **Versioning strategy:** all crates share the workspace version
  (`version.workspace`), so the three library crates move in lockstep.
  Independent versioning is possible later by giving `letratag-render` its
  own version and a `version = "0.2"`-style requirement, but lockstep is
  simpler while the protocol is still being hardware-calibrated.

## Publish order (executed 2026-10-21)

```sh
cargo publish -p letratag-protocol
cargo publish -p letratag-render      # after protocol is on crates.io
cargo publish -p letratag-transport   # independent, any time
```

Executed in exactly this order, each from a clean checkout with the token
loaded from the git-ignored `.env` (never printed or committed), with
per-crate verification: `cargo package --list` (contents), packaged
manifest inspection (metadata, dependencies), `cargo publish --dry-run`
(build verification plus authenticated dry-run upload), and after each
publish an API check that the version is visible before the next dependent
crate was packaged. All three published at 0.1.0. A first attempt was
rejected because the account email was not yet verified (HTTP 400, not an
authentication failure); after verification the same flow succeeded. The
temporary token was left for the operator to revoke.

Dry-run verification (no upload):

```sh
cargo publish --dry-run -p letratag-protocol
cargo publish --dry-run -p letratag-transport
cargo publish --dry-run -p letratag-transport --features ble
cargo package --list -p <crate>
```

## Publication record

* 2026-10-21: `letratag-protocol` 0.1.0 and `letratag-render` 0.1.0
  published (links in the table above). Verified via the crates.io API
  (`max_version` 0.1.0) and the sparse index (0.1.0, not yanked).
  `letratag-transport` was NOT published; see the correction above.
* 2026-10-21: the 0.1.1 patch publication (README-carrying packages) was
  first **blocked** by a token permission error (HTTP 403 on the first
  upload); nothing was published at 0.1.1 in that attempt. Note: `cargo
  publish --dry-run` reached the upload step, so a dry run is not a
  reliable permission check.
* 2026-10-21: after the operator updated the token, the 0.1.1
  publication completed in dependency order: `letratag-protocol`,
  `letratag-render`, `letratag-transport`. Each was verified via the
  crates.io API (`max_version` 0.1.1), the sparse index (0.1.1, not
  yanked), and an actual download of the published `.crate` (README.md
  present, version 0.1.1). The published `letratag-render` 0.1.1 depends
  on `letratag-protocol ^0.1.1` (registry dependency listing). The
  workspace version bump to 0.1.1 is commit `b210d4c`; the doc-tests
  (12 across the three crates) passed on the packaged state before
  publication. The token in the git-ignored `.env` was left for the
  operator to revoke.
* 2026-10-21: dedicated crate READMEs added for all three published crates
  (commits `0208605`, `234fa00`, `68534e1`). Each README is the crate-level
  rustdoc via `#![doc = include_str!("../README.md")]`, so every example is
  a doc-test compiled and run by `cargo test`, and is included in the
  package (`readme = "README.md"` in each manifest, confirmed with
  `cargo package --list`). The published 0.1.0 versions do not contain the
  READMEs; publishing them requires a new version.
* The recommended 0.1.1 patch publication (see above) is **done**; the
  strategy was: README and packaging-metadata additions are
  non-breaking, so a patch bump is the correct semver level. Because all
  crates share the workspace version, `[workspace.package] version` moved
  to `0.1.1` (the unpublished `letratag-printer` carries the same version
  locally and is unaffected). Published in dependency order: protocol,
  render, transport. `letratag-printer` was not published.
* Versioning note: the crate versions (0.1.x) and the application's GitHub
  release tags (v0.2.0) have diverged. The library crates are versioned
  independently of the application releases; keep the lockstep workspace
  version for the three published crates and document the mapping here
  rather than forcing the application's release cadence onto them.

## Known limitations of the published crates

* `letratag-protocol` encodes a wire format whose printer-side acceptance is
  verified only for the legacy profile on one LT200B firmware
  (`RUTEP20220628`); the modern profile is source-backed, not
  hardware-confirmed.
* `letratag-transport` never proves physical printing; completion semantics
  live in the (unpublished) printer crate's engine.
* No crate guarantees exactly-once physical output; the durable journal is
  not a cross-request idempotency service.
