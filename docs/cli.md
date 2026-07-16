# CLI reference

Global selectors are `--project`, `--sdk`, `--module`, `--variant`, `--device`,
and `--profile`. Output is `human`, `json`, or streaming `jsonl` where supported.
Status and diagnostics go to stderr; machine envelopes remain on stdout.

| Command | Purpose |
| --- | --- |
| `doctor` | Validate SDK, Java, ADB, and project tooling |
| `project inspect` | Show the detected model and support level |
| `modules list`, `variants list` | List model selections |
| `devices list`, `emulators list` | Inspect Android targets |
| `build`, `install`, `launch`, `run` | Execute the daily application loop |
| `rerun`, `reinstall`, `clean-reinstall` | Explicit repeat/destructive workflows |
| `stop`, `uninstall`, `clear-data` | Application lifecycle operations |
| `test` | Run local, instrumentation, or custom test targets |
| `logs` | Capture, filter, export, or explicitly record Logcat |
| `gradle` | Run explicit tasks through the project wrapper |
| `emulator` | Start or stop a selected AVD |
| `command` | Preview/trust/run a configured project command |

Selections must resolve exactly. DexDeck never silently chooses another device,
module, variant, process, or package after an explicit selection becomes stale.
Destructive and release/non-debuggable actions require explicit confirmation.

Use `dexdeck <command> --help` for complete flags. Stable JSON/JSONL contracts
are documented in `docs/protocols/cli-v1.md`.
