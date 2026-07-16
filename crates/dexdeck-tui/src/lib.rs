//! Terminal lifecycle, rendering, and input for DexDeck.

mod controls;
mod lifecycle;
mod logcat_workspace;
mod run_workspace;
mod shell;
mod test_workspace;
mod theme;
mod tooling_workspace;

pub use controls::{
    FocusPane, KeyChord, KeyConflict, Keymap, NamedAction, PaletteMatch, VirtualList, fuzzy_actions,
};
pub use lifecycle::{ActiveAnimation, MAX_ACTIVE_ANIMATION_FRAMES, TerminalProfile};
pub use logcat_workspace::{LogOverlay, LogWorkspaceAction, LogcatWorkspace};
pub use run_workspace::{RUN_HISTORY_LIMIT, RUN_OUTPUT_LIMIT, RunWorkspace, RunWorkspaceAction};
pub use shell::{
    DashboardLayout, LogcatBackend, LogcatBackendEvent, ShellError, ShellOptions, run,
    run_with_logcat,
};
pub use test_workspace::{TestWorkspace, TestWorkspaceAction};
pub use theme::{ColorCapability, GlyphMode, LazuliTheme, SemanticColors};
pub use tooling_workspace::{ToolCommandView, ToolingAction, ToolingTab, ToolingWorkspace};
