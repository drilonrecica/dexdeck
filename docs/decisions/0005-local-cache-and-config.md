# ADR 0005: OS-local cache and optional TOML configuration

**Status:** Accepted

## Context

Warm startup requires local caching, while ordinary project launch must not
change repository files. Shared configuration should remain reviewable.

## Decision

Store caches and user project configuration under platform-standard directories
keyed by a canonical-path namespace hash. Shared .dexdeck/config.toml is
optional and created only explicitly. All formats are versioned and atomically
replaced; shared TOML edits preserve comments.

## Consequences

Corrupt state can be ignored without blocking startup. Shared migrations need
confirmation; local migrations need backups.

## Rejected alternatives

Writing project state on launch violates read-only behavior. A database adds
operational complexity without a demonstrated need.
