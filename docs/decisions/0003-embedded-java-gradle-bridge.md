# ADR 0003: Embedded Java 17 Gradle bridge

**Status:** Accepted

## Context

Android build logic is dynamic and cannot be modeled correctly by parsing
Gradle source files. Users should still install one DexDeck executable.

## Decision

Build a small Java 17 bridge with isolated AGP 8.x and 9.x adapters. Commit a
reproducible bridge JAR, verify it against source in CI, embed it in DexDeck,
and inject it through a Gradle init plugin without modifying projects.

## Consequences

Cargo builds remain self-contained and bridge source stays reviewable. Bridge
changes must update the verified artifact and compatibility fixtures.

## Rejected alternatives

Build-file parsing is incorrect for dynamic builds. Runtime compilation adds
startup and JDK failure modes. A required two-stage build breaks normal Cargo
workflows.
