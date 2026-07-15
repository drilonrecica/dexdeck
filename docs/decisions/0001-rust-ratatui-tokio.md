# ADR 0001: Rust, Ratatui, and Tokio

**Status:** Accepted

## Context

DexDeck needs predictable resource use, cross-platform process control, an
event-driven terminal UI, and one native distributable.

## Decision

Use Rust 1.97.0 with Ratatui, Crossterm, and Tokio. Rendering is event-driven;
blocking filesystem and process work runs outside the input/render path.

## Consequences

The application can share typed core services between CLI and TUI and ship as a
native executable. Dependencies and unsafe platform code require review.

## Rejected alternatives

JavaScript terminal frameworks add runtime overhead. A synchronous render loop
would compromise idle use. A custom terminal renderer adds no product value.
