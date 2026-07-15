# Contributing to DexDeck

DexDeck accepts focused contributions that follow [SPECS.md](SPECS.md), the
accepted [architecture decisions](docs/decisions/README.md), and the current
phase in [TASKS.md](TASKS.md).

## Before changing code

- Discuss expensive-to-reverse architecture changes before implementing them.
- Keep work scoped; do not rewrite unrelated code.
- Do not add telemetry, direct networking, shell command execution, unbounded
  queues, automatic project mutation, or persistent raw logs.
- Add an ADR when a change supersedes an accepted architectural decision.

## Verification

Run before opening a pull request:

    cargo fmt --all -- --check
    cargo clippy --workspace --all-targets -- -D warnings
    cargo test --workspace --all-targets

Add tests for behavior, protocol changes, migrations, error paths, cancellation,
and bounded-resource invariants.

## Developer Certificate of Origin

Every commit must be signed off under the
[Developer Certificate of Origin 1.1](https://developercertificate.org/):

    git commit --signoff

The sign-off certifies that you have the right to submit the contribution under
the project's Apache-2.0 license. Pull requests containing unsigned commits
will not be merged.

## Review

Maintainers may decline changes that are out of scope, insufficiently tested,
unsafe, privacy-invasive, difficult to maintain, or inconsistent with the
accepted product direction.
