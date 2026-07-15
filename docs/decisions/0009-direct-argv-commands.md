# ADR 0009: Direct argv custom commands

**Status:** Accepted

## Context

Shell strings are platform-dependent and create injection and quoting risks.
Repository commands also introduce an execution path beyond Gradle.

## Decision

Represent and execute programs as argv arrays with an explicit working
directory. Project-defined commands require trust once, stored project trust,
or cancellation. Stored trust includes canonical path, Git remote when
available, and schema version.

## Consequences

Pipes and shell operators are unavailable in v0.2.0. Environment secrets are
resolved at execution time and redacted from every output.

## Rejected alternatives

Implicit shell execution is unsafe and inconsistent. A general embedded shell
is unnecessary.
