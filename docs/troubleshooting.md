# Troubleshooting

## Project or variant unavailable

Run `dexdeck project inspect` from the project or pass `--project`. Confirm the
wrapper is executable and the AGP/Gradle/Java combination is supported. A stale
cache remains usable but is labeled stale; it is never presented as current.

## SDK or ADB unavailable

Run `dexdeck doctor`. Prefer `--sdk` or `ANDROID_SDK_ROOT`. For unauthorized
devices, accept the host key on the device and rerun `dexdeck devices list`.
DexDeck never restarts ADB or chooses a different device unless explicitly told.

## Emulator does not boot

Check acceleration, disk space, AVD image availability, and `adb devices -l`.
Cancellation stops boot monitoring but intentionally leaves the emulator running.

## Build or test failed

Read the normalized diagnostic and raw bounded job tail. Retry the same Gradle
command directly through the project wrapper if plugin-specific output is lost.
Malformed JUnit reports are warnings; raw test output remains available.

## TUI rendering is damaged

Use `--ascii`, `--no-color`, or `DEXDECK_REDUCED_MOTION=1`. Ensure `LANG` names a
UTF-8 locale. SSH uses a lower redraw rate; tmux capability detection remains
conservative. If a crash leaves the terminal altered, run `reset`.

## Debug report

Use `--debug-log PATH` only when needed. The file is local, bounded, redacted,
and never uploaded automatically. Review it before sharing.
