# ADR 0006: Zero network and telemetry

**Status:** Accepted

## Context

Privacy is a conformance requirement, not a configurable product feature.

## Decision

DexDeck contains no telemetry, analytics, update checks, uploads, remote flags,
HTTP client, or direct outbound request. Gradle, package managers, SDK tools,
and user commands remain independently responsible for their behavior.

## Consequences

Dependency policy and runtime tests enforce the boundary. Updates are handled
by package managers and release channels.

## Rejected alternatives

Opt-in telemetry still introduces identifiers and network code. Automatic
updates conflict with local ownership and require a network trust system.
