# DexDeck

**A fast, private terminal control plane for Android development.**

DexDeck is being built to run the routine Android build, run, test, device,
emulator, Gradle, and Logcat loop without requiring Android Studio.

The implementation is in its foundation phase. The authoritative requirements
are in [SPECS.md](SPECS.md), and ordered work is tracked in
[TASKS.md](TASKS.md).

## Principles

- Correct and predictable behavior
- Fully local operation with no telemetry or direct network requests
- Event-driven, bounded resource use
- One native CLI and TUI over a shared application core

## Development

Install Rust 1.97.0, then run:

    cargo fmt --all -- --check
    cargo clippy --workspace --all-targets -- -D warnings
    cargo test --workspace --all-targets

DexDeck is licensed under the [Apache License 2.0](LICENSE).

DexDeck is an independent open-source project and is not affiliated with or
endorsed by Google.
