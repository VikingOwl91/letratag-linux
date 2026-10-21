# Contributing to letratag-linux

Thanks for looking at the project. This is an experimental driver: it prints
real labels, but only the **DYMO LetraTag 200B** has been physically
validated, and the project is not a production printer driver.

You do **not** need a printer to contribute. Ordinary code, documentation and
test contributions are made entirely against the mock transport.

## Scope and maturity

* Supported device: DYMO LetraTag 200B. No other model has been tested.
* Supported platform: Linux with BlueZ and D-Bus. CI runs on Ubuntu 24.04.
* The recommended workflow is Direct IPP (a loopback IPP server). The
  system CUPS integration is Experimental.
* Every hardware job ends *uncertain*: the printer reports success even with
  the lid open, so the software never claims that a label was printed.

## Prerequisites

* Rust 1.98.1, pinned in `rust-toolchain.toml` (1.98 is the minimum).
* The native dependencies listed in the
  [README requirements table](README.md#requirements).
* The patched PAPPL build, which the project builds into a private prefix:

  ```sh
  bash scripts/build-pappl.sh
  export PKG_CONFIG_PATH="$PWD/.research/pappl-prefix/lib/pkgconfig${PKG_CONFIG_PATH:+:$PKG_CONFIG_PATH}"
  ```

## Mock-only development workflow

The mock transport is the default and records the exact bytes a job would
send, so the whole pipeline can be developed and tested without a printer
and without Bluetooth:

```sh
# Mock-only build: neither bluer nor btleplug is compiled in.
cargo build --locked --release -p letratag-printer --features pappl

# Build and test everything, including the BLE backends.
cargo build --locked --workspace --all-features
cargo test --locked --workspace --all-features
```

## Build, test and lint commands

These are the commands CI runs, and they are expected to pass before a pull
request is opened:

```sh
cargo fmt --all -- --check
rustfmt --check tests/make-pdf.rs tests/ipp-client.rs tests/inspect-captures.rs tests/inspect-direct-captures.rs
cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
cargo test --locked --workspace
cargo test --locked --workspace --all-features
```

The offline suites use isolated mock servers and never transmit to
hardware:

```sh
bash scripts/test-direct-ipp.sh
bash scripts/test-ipp.sh
python3 scripts/test-forensic-raster.py
bash scripts/test-cups-geometry.sh
python3 scripts/check-docs.py
bash scripts/check-licenses.sh
```

`scripts/check-docs.py` verifies that every relative documentation link
resolves; run it whenever you edit Markdown.

## Reporting bugs and proposing features

Use the issue forms in this repository:

* **Bug report** for reproducible problems, including mock-only ones.
* **Feature request** for proposals.

A good bug report says which version or commit you used, your distribution
and kernel, the transport backend (`mock`, BlueZ or btleplug), how you
installed it, the exact steps, and what you expected versus what happened.
The form asks for hardware details only where they matter.

## Pull request expectations

* One focused change per pull request, with a description of what changed
  and why.
* Tests for the behaviour you changed. A software test proves the mapping
  and the wire bytes; it does **not** prove that a physical label was
  printed.
* `cargo fmt`, `rustfmt` and `cargo clippy -D warnings` clean, and the
  commands above passing.
* Documentation updated when behaviour, commands or limits change. The
  evidence documents under `docs/evidence/` are a dated record: do not
  rewrite them. Add a new dated document instead, or annotate the old one
  with a pointer to the newer result.
* No new dependencies without a discussion first, and no changes to the
  protocol, the calibration model or the transport behaviour that are not
  backed by evidence.

The pull request template asks whether hardware was involved and whether a
physical output was actually observed. Answering "no" is perfectly normal
for most changes.

## Hardware testing

Physical printing is opt-in, never automatic, and never part of CI or the
test suites.

* Ask before any physical print is run on someone's printer, and treat
  operator approval as required, not optional.
* A hardware job that times out may already have printed. Do not resubmit
  the label; follow the journal procedure in
  [docs/security.md](docs/security.md).
* After a hardware run, record what was measured, in a dated document under
  `docs/evidence/`, and say clearly whether a physical label was actually
  observed. "The job returned success" is not the same as "a label came
  out".

## Reporting hardware observations

If you contribute a hardware observation, remove anything identifying:
Bluetooth addresses, hostnames, usernames, home directory paths, and
journal or capture file contents. Label geometry measurements and printer
model information are what the project needs.

If a hardware run leaves an unresolved journal, keep it and describe it;
do not delete the state files to make the queue usable again.
