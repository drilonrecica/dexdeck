# v0.2.0 acceptance mapping

This document maps every acceptance area in `SPECS.md` section 33 to an
automated test or reproducible release check. A mapping is not a passing gate;
the final tag requires green results on the exact candidate commit.

| Acceptance area | Automated/reproducible evidence |
| --- | --- |
| Nested discovery, wrappers, modules, flavors, disabled variants | `modeling_matrix`, real AGP matrix, bridge reproducibility jobs |
| Warm cache, refresh, corruption, degraded mode | config/cache focused suite, model service/refresh tests, reliability job |
| Build/install/launch/stop/rerun/reinstall/clean reinstall | application/workflow tests and scheduled emulator daily-loop harness |
| Exact device/emulator selection | Android device/emulator tests and scheduled ADB/emulator jobs |
| Application Logcat and secondary processes | Logcat parser/service process tests and fake ADB reconnect fixture |
| Logcat throughput, bounds, filters, export, recording | release stress job, Criterion pipeline, core Logcat tests |
| Local and instrumentation tests, selection, failures, reruns | test runner/result fixtures plus scheduled instrumentation harness |
| CLI/TUI parity and versioned machine output | protocol goldens, CLI flow tests, TUI TestBackend snapshots |
| No telemetry/network/upload/default raw persistence | privacy contract, cargo-deny, threat model, Linux syscall smoke |
| Secret redaction and command trust | core secret/custom-command tests and direct-argv source audit |
| Cancellation and orphan prevention | process group/Windows Job Object tests, reconnect cancellation tests |
| Exit safety and terminal restoration | terminal PTY test and active-job lifecycle snapshots |
| Normal launch leaves Git unchanged | modeling fixture inventory/status test and CI clean-tree checks |
| Five target archives, installers, checksums, provenance | release workflow, artifact verifier, package smoke workflow |
| Homebrew prebuilt installation | formula generator self-test and two-macOS tap smoke matrix |

Release evidence must record the workflow URLs, commit, toolchain, Android SDK,
AGP/Gradle lanes, benchmark metadata, artifact digests, and Homebrew tap commit.
