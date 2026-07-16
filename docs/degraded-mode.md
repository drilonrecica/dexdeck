# Degraded mode

DexDeck enters degraded mode when complete AGP-backed modeling is unavailable,
including unsupported AGP, a missing/broken wrapper, bridge failure, or a
non-Android directory. It reports the reason and keeps the last valid model
when one exists.

Device-wide operations, doctor checks, explicit Gradle tasks, and device-scope
Logcat may remain available. Variant-derived application IDs, artifacts,
install/launch actions, and application-scoped Logcat remain unavailable until
the required model selection is known. DexDeck does not guess these values by
parsing build scripts or selecting a different project.

Run `dexdeck doctor` and `dexdeck project inspect --format json` to identify the
missing capability. Fix the wrapper/toolchain or use explicit task/device
operations that do not require unavailable model data.
