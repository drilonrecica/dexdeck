# Releases

## Unreleased

No changes after the 0.2.0 release candidate.

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
