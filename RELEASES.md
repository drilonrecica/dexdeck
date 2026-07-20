# Releases

## Unreleased

No changes after 0.2.2.

## 0.2.2 — 2026-07-20

DexDeck 0.2.2 fixes project modeling for large or metadata-rich Android builds
whose valid bridge output used JSON escaping different from Rust's serializer.

### Fixed

- Hash the exact emitted Gradle bridge records instead of independently
  serialized Java and Rust project models, preventing false `model hash
  mismatch` failures while retaining output-integrity validation.
- Add real AGP regression coverage for Unicode task metadata across the
  supported AGP 8 and 9 compatibility lanes.
- Support the Bash 3.2 environment used by macOS package-smoke runners.

### Compatibility

The bridge wire shape and schema remain at version 1. Its content-addressed
cache changes automatically, so users do not need to clear existing caches.
Windows x86-64 remains experimental.

## 0.2.1 — 2026-07-20

DexDeck 0.2.1 modernizes the terminal interface and repairs the release
validation path used after 0.2.0.

### Changed

- Replaced the nested boxed dashboard with a border-light Lazuli canvas,
  seven-workspace top navigation, contextual guidance, and responsive layouts.
- Added selectable command, help, and search overlays plus consistent keyboard
  and mouse navigation.
- Changed normal-screen exit behavior: `Esc` closes or returns, while `q` and
  `Ctrl+C` request a safe exit.
- Preserved true-color, 256-color, ANSI-16, monochrome, light-background,
  Unicode, and ASCII rendering fallbacks.

### Fixed

- Restored manual package-smoke dispatch and corrected Android emulator setup
  and ADB resolution in release validation.
- Made release artifact versions, titles, Homebrew commits, and readiness checks
  derive from the release version instead of the original 0.2.0 constants.

### Known limitations

Run, test, device, task, and doctor actions are shown honestly as unavailable in
the TUI until their service adapters are connected. Their CLI equivalents remain
available. Windows x86-64 is experimental.

## 0.2.0 — release candidate

DexDeck 0.2.0 establishes the first complete local Android terminal loop.

### Included

- AGP-backed project models for the pinned AGP 8 and 9 compatibility lanes,
  atomic caches, refresh watchers, and explicit degraded mode.
- Exact SDK/device/emulator/profile selection and supervised build, install,
  launch, rerun, reinstall, stop, clear, test, and trusted-command workflows.
- Structured, byte-bounded Logcat with process tracking, filters, crash markers,
  export, explicit recording, CLI JSONL, and a responsive TUI workspace.
- Local/instrumentation test selection, structured failures, reruns, normalized
  diagnostics, and direct-argv editor navigation.
- Responsive Lazuli TUI, Deckmark assets, shell completions, man page, release
  archives, checksummed installers, provenance, and Homebrew tap automation.

### Security and privacy

There is no telemetry, direct network client, update checker, upload path, or
automatic raw-log persistence. Runtime dependency/source assertions and Linux
socket-syscall smoke tests enforce this boundary. Project commands require
explicit fingerprinted trust; secret values are redacted.

### Compatibility

Linux and macOS are stable targets. Windows x86-64 is experimental: compilation,
direct process execution, Job Object cleanup, paths, archives, PowerShell
installation, and terminal restoration are tested in CI, but real-world device
and terminal coverage is less mature. WSL-to-host ADB forwarding is deferred.

The final `v0.2.0` tag must not be created until all quality, platform, Android,
AGP, stress, privacy, reliability, package, and Homebrew release-candidate jobs
pass on the exact commit.
