//! Terminal lifecycle, rendering, and input for DexDeck.

mod shell;
mod theme;

pub use shell::{ShellError, ShellOptions, run};
pub use theme::{ColorCapability, GlyphMode, LazuliTheme, SemanticColors};
