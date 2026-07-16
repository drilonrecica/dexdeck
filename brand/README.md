# Deckmark assets

The Deckmark is two interlocking geometric D forms representing terminal panes,
stacked control surfaces, variants, and DEX layers. Geometry uses filled paths,
not fonts, strokes, gradients, embedded data, or external resources.

Clear space is one eighth of the compact mark width on every side. Do not add
Android/Google imagery, antennae, playing-card symbols, franchise references,
mascots, phones, generic code brackets, gradients, or effects. The monochrome
forms prove that recognition does not depend on color.

Canonical terminal-only fallbacks are `[DD] DexDeck` and `▰▱ DexDeck`.

Run `scripts/render-brand-assets.sh` after changing SVG geometry. It renders
fixed-size PNGs, strips variable metadata, and updates `SHA256SUMS`. CI reruns
the script and requires byte-identical output.
