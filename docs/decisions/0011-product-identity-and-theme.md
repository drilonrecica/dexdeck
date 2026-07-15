# ADR 0011: DexDeck identity and Lazuli theme

**Status:** Accepted

## Context

Product strings, paths, protocols, documentation, and visual assets can drift
across a multi-crate project.

## Decision

Centralize DexDeck, dexdeck, DEXDECK_, .dexdeck, repository, and Lazuli
identifiers. Use semantic theme tokens and the geometric Deckmark constraints.
Do not use Google/Android robot, franchise, casino, crypto, or mascot imagery.

## Consequences

Brand constants become shared contracts. Terminal operation remains readable
without color, images, bundled fonts, or Nerd Font glyphs.

## Rejected alternatives

Scattered literals create migration risk. Decorative or derivative branding
conflicts with operational clarity and legal independence.
