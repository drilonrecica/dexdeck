# ADR 0004: Versioned JSON and JSONL protocols

**Status:** Accepted

## Context

The bridge and CLI expose machine-consumed data that must not change silently.
Streaming output must coexist with arbitrary child-process output.

## Decision

Use schema-versioned JSON for snapshots and JSON Lines for streams. Every
record carries its version. Bridge records use an explicit temporary file and
a completion record; stdout and stderr remain separate.

## Consequences

Protocol changes require compatibility tests and documentation. Incompatible
versions and incomplete bridge files fail explicitly.

## Rejected alternatives

Mixing structured records into Gradle stdout is fragile. An unversioned schema
cannot provide compatibility. A binary protocol is unnecessary for v0.2.0.
