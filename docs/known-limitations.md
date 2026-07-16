# Known limitations in 0.2.0

- Windows x86-64 is experimental; terminal, path, wrapper, and Job Object
  behavior is compiled and tested but has less field exposure than Linux/macOS.
- WSL-to-Windows emulator forwarding and container-to-host ADB bridging are not
  first-class workflows.
- AGP older than 8 or 10 and newer uses degraded task mode.
- No remote device farm, cloud build, IDE protocol, graphical debugger,
  profiler, manifest-merger UI, or SARIF dashboard is included.
- Crates.io publication is intentionally deferred; use archives, Homebrew, or a
  source checkout.
- DexDeck does not install SDK components, Java, Gradle, project dependencies,
  or emulator images during normal product operation.

These are scope boundaries, not reasons to silently guess project/device state.
