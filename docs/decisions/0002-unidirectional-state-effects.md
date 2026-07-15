# ADR 0002: Unidirectional state and effects

**Status:** Accepted

## Context

UI input, Gradle jobs, devices, watchers, and Logcat update concurrently.
Uncontrolled shared mutation would make cancellation and recovery unreliable.

## Decision

Use input to action to pure reducer to effect to result-action flow. Workers
communicate through bounded channels and never mutate application state.

## Consequences

Reducers are replayable and testable. Effects require explicit lifecycle,
backpressure, and cancellation definitions.

## Rejected alternatives

Shared mutable service state and widget-owned I/O obscure ordering and failure
handling. A general actor framework adds unnecessary abstraction.
