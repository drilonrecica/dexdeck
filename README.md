<p align="center"><img src="brand/deckmark-horizontal.svg" alt="DexDeck" width="360"></p>

# DexDeck

**A fast, private terminal control plane for Android development.**

DexDeck lets you build, run, test, inspect Logcat, and manage Android devices
without depending on a heavyweight IDE. It provides one native CLI and TUI over
the same bounded, event-driven core.

- Local-only: no telemetry, update checks, uploads, or direct network requests.
- Predictable: exact project, variant, and device selection; no silent fallback.
- Editor-independent: direct source navigation for terminal and GUI editors.

## Install

Version 0.2.1 release archives target Linux x86-64/aarch64, macOS x86-64/Apple
Silicon, and experimental Windows x86-64. Until artifacts are published, build
from a checkout with Rust 1.97:

```sh
cargo build --release -p dexdeck
install -m 755 target/release/dexdeck ~/.local/bin/dexdeck
```

See [installation](docs/installation.md) for archives, installers, Homebrew,
shell completions, platform requirements, and checksum verification.

## Quick start

From any directory inside an Android Gradle project:

```sh
dexdeck doctor
dexdeck project inspect
dexdeck modules list
dexdeck variants list --module :app
dexdeck devices list
dexdeck run --module :app --variant debug --device emulator-5554
dexdeck logs --scope application --module :app --variant debug
dexdeck test --kind local --module :app --variant debug
```

Run `dexdeck` without a subcommand in an interactive terminal to open the TUI.
DexDeck uses the project Gradle wrapper and standard Android SDK discovery.

## Documentation

- [CLI reference](docs/cli.md)
- [Configuration](docs/configuration.md)
- [AGP compatibility](docs/agp-compatibility.md)
- [Degraded mode](docs/degraded-mode.md)
- [Privacy and threat model](docs/privacy.md)
- [Troubleshooting](docs/troubleshooting.md)
- [Known limitations](docs/known-limitations.md)
- [Release process](docs/release.md)

The authoritative requirements are in [SPECS.md](SPECS.md). Ordered delivery
work is tracked in [TASKS.md](TASKS.md). Reproducible performance methodology
is documented under [benchmarks](benchmarks/README.md).

## Development

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
```

DexDeck is licensed under the [Apache License 2.0](LICENSE). It is an independent
open-source project and is not affiliated with or endorsed by Google.
