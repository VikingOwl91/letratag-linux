# crates.io publication

Status: **published 2026-10-21.** The three library crates are on crates.io
at version 0.1.0 (operator-approved; see the publication record below).
`letratag-printer` is deliberately not published.

Assessment date: 2026-10-21, workspace at `95bf1f0`; publication executed
from `c811dd7` (clean tree, git-ignored `.env` token, no credentials
committed).

## Crate-by-crate assessment

| Crate | Published | Version | crates.io |
|---|---|---|---|
| `letratag-protocol` | yes, 2026-10-21 | 0.1.0 | <https://crates.io/crates/letratag-protocol> |
| `letratag-render` | yes, 2026-10-21 | 0.1.0 | <https://crates.io/crates/letratag-render> |
| `letratag-transport` | yes, 2026-10-21 | 0.1.0 | <https://crates.io/crates/letratag-transport> |
| `letratag-printer` | **no** | — | the `pappl` feature builds against the pinned, patched PAPPL via pkg-config (`scripts/build-pappl.sh`, private prefix); a crates.io build cannot reproduce that, so the published crate would not build its main feature from source |

`letratag-printer` is an internal application crate: the PAPPL/IPP service,
the CLI, the C adapter and the private patched-PAPPL build all belong to
the repository, not to a registry package. Keep it unpublished.

## Readiness review

* **Names:** `letratag-protocol`, `letratag-render`, `letratag-transport` are
  available on crates.io (verified by the index lookup during the dry runs).
* **License:** MIT for all crates (`license.workspace`). The packaged
  archives do not embed the LICENSE text (it lives at the repository root,
  outside the crate directories). Optional improvement before publishing:
  copy `LICENSE` into each published crate.
* **Metadata:** all four manifests now carry `description` and
  `repository`. `documentation`/`homepage` are unset; docs.rs links appear
  automatically after the first publish.
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

* 2026-10-21: `letratag-protocol` 0.1.0, `letratag-render` 0.1.0,
  `letratag-transport` 0.1.0 published (links in the table above). Verified
  via the crates.io API (`max_version` 0.1.0 for each).
* 2026-10-21: dedicated crate READMEs added for all three published crates
  (commits `0208605`, `234fa00`, `68534e1`). Each README is the crate-level
  rustdoc via `#![doc = include_str!("../README.md")]`, so every example is
  a doc-test compiled and run by `cargo test`, and is included in the
  package (`readme = "README.md"` in each manifest, confirmed with
  `cargo package --list`). The published 0.1.0 versions do not contain the
  READMEs; publishing them requires a new version.
* Recommended next publication: **0.1.1** (patch) of the three library
  crates. README and packaging-metadata additions are non-breaking, so a
  patch bump is the correct semver level. Because all crates share the
  workspace version, bump `[workspace.package] version` to `0.1.1` (the
  unpublished `letratag-printer` carries the same version locally and is
  unaffected), then publish in dependency order: protocol, render,
  transport. Do not publish `letratag-printer`.
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
