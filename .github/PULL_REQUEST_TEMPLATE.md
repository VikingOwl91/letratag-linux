<!-- Keep this short. Every field can be answered "no" or "not applicable". -->

## What changed

<!-- One or two sentences. -->

## Why

<!-- The problem this solves, or the reference to the issue. -->

## Tests performed

<!-- The commands you ran, and their result. -->

```text
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
cargo test --locked --workspace --all-features
```

## Hardware

- [ ] Real hardware was involved in testing (a job reached a physical printer).
- [ ] A physical label was actually observed on the printer.
- [ ] Not applicable: the change was verified with the mock transport only.

<!-- Hardware validation is not required for documentation, mock-only or
     other software changes. Answer honestly: a software test never proves
     that a physical label was printed. -->

## Documentation

- [ ] Documentation was updated, or no user-visible behaviour changed.
- [ ] Evidence documents under `docs/evidence/` were left as recorded; new
      results are in a new dated document.

## Safety

- [ ] No unintended physical printing was performed.
- [ ] No protocol, transport or calibration behaviour changed without
      evidence, and any change is explained above.
