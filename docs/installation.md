# Installation

## Requirements

- Android SDK platform-tools for device operations.
- Java 17 or the Java runtime required by the selected Gradle wrapper.
- A Gradle wrapper in the project for full project modeling and build actions.
- A terminal at least 40×10 for the TUI; the CLI works in smaller terminals.

SDK discovery checks `--sdk`, configuration, the project, `ANDROID_SDK_ROOT`,
legacy `ANDROID_HOME`, and platform defaults in documented precedence order.

## Release archives

Download the archive matching the host from the GitHub release. Download
`SHA256SUMS`, then verify before extracting:

```sh
sha256sum --check SHA256SUMS
tar -xzf dexdeck-0.2.2-x86_64-unknown-linux-gnu.tar.gz
install -m 755 dexdeck-0.2.2-x86_64-unknown-linux-gnu/dexdeck ~/.local/bin/
```

On Windows use `Get-FileHash -Algorithm SHA256`, extract the ZIP, and place
`dexdeck.exe` on `PATH`. Windows support is experimental in 0.2.2.

The POSIX and PowerShell installers download only from the project’s GitHub
release and verify the published checksum before installation. Review scripts
before piping or executing them.

## Homebrew

After the tap release is published:

```sh
brew install drilonrecica/tap/dexdeck
```

The formula installs a checksummed prebuilt artifact; Rust is not required.

## Shell integration

Reviewed completion files are in `completions/`. Install the appropriate file
in the shell’s standard completion directory. The manual page is `man/dexdeck.1`.

DexDeck performs no update checks. Upgrade through the same package manager or
release channel used for installation.
