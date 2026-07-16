# Privacy

DexDeck is local-only. It contains no analytics, telemetry, crash upload,
remote flags, update client, HTTP client, or direct outbound request. It does
not automatically persist raw Logcat, build output, or device data.

Disk writes are limited to explicit configuration changes, bounded local cache
and session state, job metadata, debug logs explicitly requested by path,
Logcat exports, and explicit recordings. Sensitive environment values are
redacted before diagnostics and never enter serialized UI/job state.

Gradle, package managers, SDK tools, ADB, emulators, editors, and user-trusted
custom commands are separate programs and may use the network independently.
DexDeck launches them with direct argv and does not claim control over their
privacy behavior.

See the [threat model](security/threat-model.md), dependency policy, and
zero-network ADR for exact controls and non-goals.
