# ADR 0007: Serialize mutating Gradle operations per root

**Status:** Accepted

## Context

Concurrent Gradle invocations against one build can contend for state and make
job outcomes difficult to predict.

## Decision

Allow one mutating Gradle operation per canonical primary root and queue later
operations. Device tracking, Logcat, and other non-Gradle work may continue.
Gradle retains its own internal parallelism.

## Consequences

Scheduling is predictable and cancellation has one owner. Multiple DexDeck
processes are not globally serialized and must rely on Gradle's own locking.

## Rejected alternatives

Unrestricted parallel Gradle execution optimizes an uncommon case at the cost
of reliability. A persistent coordination daemon is outside v0.2.0.
