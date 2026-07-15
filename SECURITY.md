# Security policy

## Supported versions

DexDeck has not published a supported release yet. After v0.2.0, the latest
release line will receive security fixes.

## Reporting a vulnerability

Use GitHub's private vulnerability reporting for
drilonrecica/dexdeck. If private reporting is unavailable, open a public issue
requesting a private contact channel without including sensitive details.

Do not include secrets, private project data, raw build output, or Logcat data
in a public report.

## Security and privacy boundary

DexDeck itself makes no direct network requests and contains no telemetry,
automatic update checks, crash uploads, or background service. Gradle, Android
SDK tools, package managers, and user-defined child commands may independently
use the network.
