//! Terminal lifecycle, rendering, and input for DexDeck.

mod controls;
mod logcat_workspace;
mod shell;
mod test_workspace;
mod theme;

pub use controls::{
    FocusPane, KeyChord, KeyConflict, Keymap, NamedAction, PaletteMatch, VirtualList, fuzzy_actions,
};
pub use logcat_workspace::{LogOverlay, LogWorkspaceAction, LogcatWorkspace};
pub use shell::{
    DashboardLayout, LogcatBackend, LogcatBackendEvent, ShellError, ShellOptions, run,
    run_with_logcat,
};
pub use test_workspace::{TestWorkspace, TestWorkspaceAction};
pub use theme::{ColorCapability, GlyphMode, LazuliTheme, SemanticColors};
