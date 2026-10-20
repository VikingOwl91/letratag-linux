# crates.io publication plan

Status: **prepared, not published.** Publishing requires explicit operator
approval and a crates.io token owned by the operator. Nothing in this
document has been uploaded.

Assessment date: 2026-10-21, workspace at `95bf1f0`.

## Crate-by-crate assessment

| Crate | Publish? | Dry-run | Blockers |
|---|---|---|---|
| `letratag-protocol` | yes | passes (build verified) | approval, crates.io token |
| `letratag-render` | yes | manifest ready | needs `letratag-protocol` on crates.io first (path dependency now carries `version = "0.1.0"`, so packaging resolves it from the index) |
| `letratag-transport` | yes | passes, default and `ble` features | approval, crates.io token |
| `letratag-printer` | **no** | not attempted | the `pappl` feature builds against the pinned, patched PAPPL via pkg-config (`scripts/build-pappl.sh`, private prefix); a crates.io build cannot reproduce that, so the published crate would not build its main feature from source |

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

## Publish order (after approval)

```sh
cargo publish -p letratag-protocol
cargo publish -p letratag-render      # after protocol is on crates.io
cargo publish -p letratag-transport   # independent, any time
```

Dry-run verification used during preparation (no upload):

```sh
cargo publish --dry-run -p letratag-protocol --allow-dirty
cargo publish --dry-run -p letratag-transport --allow-dirty
cargo publish --dry-run -p letratag-transport --allow-dirty --features ble
cargo package --list -p <crate> --allow-dirty
```

`--allow-dirty` was needed only because the preparation ran on a dirty
working tree; publish from a clean checkout instead.

## Known limitations of the published crates

* `letratag-protocol` encodes a wire format whose printer-side acceptance is
  verified only for the legacy profile on one LT200B firmware
  (`RUTEP20220628`); the modern profile is source-backed, not
  hardware-confirmed.
* `letratag-transport` never proves physical printing; completion semantics
  live in the (unpublished) printer crate's engine.
* No crate guarantees exactly-once physical output; the durable journal is
  not a cross-request idempotency service.
