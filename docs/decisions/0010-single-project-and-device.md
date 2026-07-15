# ADR 0010: One project and one active device per process

**Status:** Accepted

## Context

Multiple roots and active devices multiply state, ownership, queueing, and
Logcat scope complexity.

## Decision

Each DexDeck process owns one primary Gradle root and one active interactive
device. Users may switch the device or run more DexDeck processes.

## Consequences

Selections, jobs, and application-scoped logs remain unambiguous. Batch
multi-device execution is deferred.

## Rejected alternatives

Multi-root workspaces and simultaneous device matrices delay the reliable daily
loop and require more complex supervision.
