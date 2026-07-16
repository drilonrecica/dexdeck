//! Terminal lifecycle, rendering, and input for DexDeck.

mod logcat_workspace;
mod shell;
mod theme;

pub use logcat_workspace::{LogOverlay, LogWorkspaceAction, LogcatWorkspace};
pub use shell::{
    LogcatBackend, LogcatBackendEvent, ShellError, ShellOptions, run, run_with_logcat,
};
pub use theme::{ColorCapability, GlyphMode, LazuliTheme, SemanticColors};
