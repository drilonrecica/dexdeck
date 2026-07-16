# DexDeck threat model

## Security boundary

DexDeck is a local process operating on one user-selected Android project and
the Android/Gradle tools that user already controls. It does not provide a
network service, cross-user service, privilege boundary, sandbox, or secret
store. Release infrastructure and child tools are outside the product runtime.

## Protected assets

- Project source, build output, signing-related environment values, device
  identifiers, application logs, command arguments, and local configuration.
- Terminal integrity, bounded memory/disk use, and ownership of child process
  trees started by the current DexDeck session.

## Credible threats and controls

| Threat | Control |
| --- | --- |
| Accidental outbound traffic or telemetry | No HTTP/TLS dependency, Tokio networking disabled, source/dependency assertions, Linux socket-syscall smoke test |
| Secret disclosure | Sensitive values are typed and redacted; command debug output contains only counts and environment keys |
| Malicious project command | Commands require explicit trust bound to project identity, remote, argv, working directory, and environment names |
| Shell injection | Every product command uses direct argv; no shell parsing or interpolation |
| Cache/config replacement or symlink attack | Private atomic files, symlink/reparse rejection, schema envelopes, corruption quarantine |
| Unbounded hostile output | Byte-bounded process, diagnostic, job, recording, and Logcat queues/buffers |
| Orphaned child processes | Supervised owned process groups/Windows Job Objects with graceful then forced cancellation |
| Unexpected persistence | Raw logs are written only after explicit export or recording actions; no background spool |
| Terminal corruption | RAII and panic hooks restore raw mode, alternate screen, cursor, and mouse capture |

## Explicit non-goals

DexDeck does not defend against a compromised operating system, Android SDK,
Gradle wrapper, emulator, ADB server, editor, package manager, or explicitly
trusted custom command. Those child tools may use the network independently;
DexDeck must not claim their behavior as its own or silently grant trust.

## Release audit

Every release runs dependency policy, static privacy assertions, source scans,
file-permission tests, redaction tests, and a socket-syscall smoke test. Any
runtime network feature requires a specification revision and accepted ADR.
