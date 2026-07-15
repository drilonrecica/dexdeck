# Dependency and network-capability policy

DexDeck is local-only software. Product dependencies must not introduce an HTTP,
TLS, WebSocket, QUIC, or network RPC stack. The deny list in the repository is
a release gate, not a complete security model.

## Adding a dependency

Before adding a normal dependency:

1. Confirm the standard library or an existing dependency cannot reasonably do
   the job.
2. Review direct and transitive features with cargo tree.
3. Disable default features that add unused capabilities.
4. Check maintenance, advisories, license, source, unsafe code, and platform
   behavior.
5. Add tests for failure, cancellation, bounds, and redaction where relevant.
6. Document why a socket-capable low-level crate is required.

Platform runtimes may contain generic I/O primitives that are technically able
to open sockets. They are acceptable only when DexDeck does not enable or call
their networking APIs. Tokio's net feature is not enabled.

Build, CI, release, Gradle, Android SDK, package-manager, and user-command
processes may independently use the network. They must not become product
runtime dependencies or be represented as DexDeck-originated requests.

Any exception to a denied crate requires a specification change, an accepted
ADR, explicit maintainer approval, and privacy/security tests.
