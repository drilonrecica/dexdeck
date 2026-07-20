use std::{
    io::{self, Stdout, Write},
    panic,
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};

use crossterm::{
    cursor::{Hide, Show},
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyEventKind,
        KeyModifiers, MouseButton, MouseEventKind,
    },
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Frame, Terminal,
    backend::{Backend, CrosstermBackend},
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Modifier, Style},
    symbols::border,
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
};

use dexdeck_protocol::{
    DegradedReason, LogRecord, ModelFreshness, ProjectSnapshot, ProjectSupport,
};

use crate::{
    ActiveAnimation, ColorCapability, FocusRegion, GlyphMode, LazuliTheme, LogOverlay,
    LogWorkspaceAction, LogcatWorkspace, NamedAction, OverviewWorkspace, RunWorkspace,
    TerminalProfile, TestWorkspace, ToolingTab, ToolingWorkspace, WorkspaceId, fuzzy_actions,
};

pub const MINIMUM_WIDTH: u16 = 40;
pub const MINIMUM_HEIGHT: u16 = 10;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DashboardLayout {
    Full,
    Compact,
    SingleWorkspace,
    ResizeWarning,
}

impl DashboardLayout {
    #[must_use]
    pub const fn for_size(width: u16, height: u16) -> Self {
        if width < MINIMUM_WIDTH || height < MINIMUM_HEIGHT {
            Self::ResizeWarning
        } else if width >= 120 && height >= 24 {
            Self::Full
        } else if width >= 80 && height >= 16 {
            Self::Compact
        } else {
            Self::SingleWorkspace
        }
    }
}

const ASCII_BORDER: border::Set = border::Set {
    top_left: "+",
    top_right: "+",
    bottom_left: "+",
    bottom_right: "+",
    vertical_left: "|",
    vertical_right: "|",
    horizontal_top: "-",
    horizontal_bottom: "-",
};

const MAX_LOG_NOTIFICATIONS_PER_TICK: usize = 8;
const MAX_PROJECT_NOTIFICATIONS_PER_TICK: usize = 4;

#[derive(Clone, Debug)]
pub enum ProjectBackendEvent {
    Detected {
        root: PathBuf,
        wrapper_found: bool,
    },
    Model(Box<ProjectSnapshot>),
    Error {
        root: Option<PathBuf>,
        message: String,
    },
}

pub trait ProjectBackend: Send {
    fn start(&mut self) -> Result<(), String>;
    fn try_recv(&mut self) -> Option<ProjectBackendEvent>;
    /// Stops only project discovery or refresh work owned by this shell.
    fn stop(&mut self);
}

#[derive(Clone, Debug)]
pub enum LogcatBackendEvent {
    Records(Vec<LogRecord>),
    Status(String),
    Error(String),
    Recording(bool),
    Exporting(bool),
}

pub trait LogcatBackend: Send {
    fn start(&mut self) -> Result<(), String>;
    fn try_recv(&mut self) -> Option<LogcatBackendEvent>;
    fn set_device_scope(&mut self, device_scope: bool) -> Result<(), String>;
    fn select_process(&mut self) -> Result<(), String>;
    fn copy(&mut self, grouped: bool) -> Result<(), String>;
    fn export(&mut self) -> Result<(), String>;
    fn toggle_recording(&mut self) -> Result<(), String>;
    /// Only jobs owned by this DexDeck session belong here. External ADB
    /// servers, emulators, and Gradle daemons must never be included.
    fn active_foreground_jobs(&self) -> usize {
        0
    }
    fn cancel_foreground_jobs(&mut self) -> Result<(), String> {
        Ok(())
    }
    fn stop(&mut self);
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ShellOptions {
    pub no_color: bool,
    pub ascii: bool,
}

#[derive(Debug, Default)]
struct ShellState {
    input_count: u64,
    last_size: Option<(u16, u16)>,
    active_workspace: WorkspaceId,
    project_name: Option<String>,
    project_phase: ProjectPhase,
    logcat_started: bool,
    overview: OverviewWorkspace,
    logcat: LogcatWorkspace,
    tests: TestWorkspace,
    run: RunWorkspace,
    tooling: ToolingWorkspace,
    overlay: ControlOverlay,
    overlay_query: String,
    overlay_selected: usize,
    focus: FocusRegion,
    exit_prompt_jobs: Option<usize>,
    notice: String,
    hit_regions: Vec<HitRegion>,
    dirty: bool,
    animation: ActiveAnimation,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum ProjectPhase {
    #[default]
    Detecting,
    Detected,
    Available,
    Unavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct HitRegion {
    area: Rect,
    workspace: WorkspaceId,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum ControlOverlay {
    #[default]
    None,
    Palette,
    Help,
    Search,
}

pub fn run(options: ShellOptions) -> Result<(), ShellError> {
    run_with_backends(options, None, None)
}

pub fn run_with_logcat(
    options: ShellOptions,
    logcat: Option<Box<dyn LogcatBackend>>,
) -> Result<(), ShellError> {
    run_with_backends(options, None, logcat)
}

pub fn run_with_backends(
    options: ShellOptions,
    mut project: Option<Box<dyn ProjectBackend>>,
    mut logcat: Option<Box<dyn LogcatBackend>>,
) -> Result<(), ShellError> {
    let profile = TerminalProfile::detect();
    let theme = LazuliTheme::new(
        ColorCapability::detect(options.no_color),
        profile.glyph_mode(options.ascii),
    );
    let mut session = TerminalSession::enter()?;
    #[cfg(debug_assertions)]
    if std::env::var_os("DEXDECK_INTERNAL_TEST_PANIC_AFTER_ENTER").is_some() {
        panic!("injected terminal restoration test panic");
    }
    let result = run_loop(
        session.terminal_mut(),
        theme,
        profile,
        &mut project,
        &mut logcat,
    );
    if let Some(project) = &mut project {
        project.stop();
    }
    if let Some(logcat) = &mut logcat {
        logcat.stop();
    }
    result
}

fn run_loop<B: Backend>(
    terminal: &mut Terminal<B>,
    theme: LazuliTheme,
    profile: TerminalProfile,
    project: &mut Option<Box<dyn ProjectBackend>>,
    backend: &mut Option<Box<dyn LogcatBackend>>,
) -> Result<(), ShellError> {
    let mut state = ShellState::default();
    if let Some(project) = project.as_deref_mut() {
        if let Err(error) = project.start() {
            apply_project_event(
                &mut state,
                ProjectBackendEvent::Error {
                    root: None,
                    message: error,
                },
            );
        }
    } else {
        state.project_phase = ProjectPhase::Unavailable;
        state.overview.model_status = "Project detection unavailable".into();
        state.run.model_status = state.overview.model_status.clone();
    }
    terminal.draw(|frame| render(frame, &mut state, theme))?;
    let mut last_draw = Instant::now();
    loop {
        if let Some(project) = project.as_deref_mut() {
            drain_project_notifications(&mut state, project);
        }
        if state.active_workspace == WorkspaceId::Logcat
            && let Some(backend) = backend.as_deref_mut()
        {
            drain_logcat_notifications(&mut state, backend);
        }
        if event::poll(Duration::from_millis(16))? {
            let event = event::read()?;
            state.dirty = true;
            let overlay_open = state.overlay != ControlOverlay::None;
            if state.exit_prompt_jobs.is_some() {
                match event {
                    Event::Key(KeyEvent {
                        code: KeyCode::Char('y') | KeyCode::Char('q'),
                        kind: KeyEventKind::Press,
                        ..
                    }) => return Ok(()),
                    Event::Key(KeyEvent {
                        code: KeyCode::Char('c'),
                        kind: KeyEventKind::Press,
                        ..
                    }) => {
                        if let Some(backend) = backend.as_deref_mut() {
                            backend
                                .cancel_foreground_jobs()
                                .map_err(ShellError::Backend)?;
                        }
                        return Ok(());
                    }
                    Event::Key(KeyEvent {
                        code: KeyCode::Char('n') | KeyCode::Esc,
                        kind: KeyEventKind::Press,
                        ..
                    }) => state.exit_prompt_jobs = None,
                    _ => {}
                }
                continue;
            }
            if should_exit(&event) && !overlay_open {
                if request_exit(&mut state, backend.as_deref()) {
                    return Ok(());
                }
                continue;
            }
            match event {
                Event::Resize(width, height) => {
                    state.last_size = Some((width, height));
                    state.logcat.dirty = true;
                }
                Event::Key(KeyEvent {
                    code: KeyCode::Char('p'),
                    modifiers: KeyModifiers::CONTROL,
                    kind: KeyEventKind::Press,
                    ..
                }) if state.overlay == ControlOverlay::None => {
                    state.overlay = ControlOverlay::Palette;
                    state.overlay_query.clear();
                    state.overlay_selected = 0;
                }
                Event::Key(KeyEvent {
                    code: KeyCode::Char('?'),
                    kind: KeyEventKind::Press,
                    ..
                }) if state.overlay == ControlOverlay::None => state.overlay = ControlOverlay::Help,
                Event::Key(KeyEvent {
                    code: KeyCode::Char('/'),
                    kind: KeyEventKind::Press,
                    ..
                }) if matches!(
                    state.active_workspace,
                    WorkspaceId::Logcat | WorkspaceId::Devices | WorkspaceId::Tasks
                ) && state.overlay == ControlOverlay::None =>
                {
                    state.overlay = ControlOverlay::Search;
                    state.overlay_query.clear();
                    apply_search_query(&mut state);
                }
                Event::Key(KeyEvent {
                    code: KeyCode::Char('/'),
                    kind: KeyEventKind::Press,
                    ..
                }) if state.overlay == ControlOverlay::None => {
                    state.notice = "Search is not available in this workspace.".into();
                }
                Event::Key(KeyEvent {
                    code: KeyCode::Esc,
                    kind: KeyEventKind::Press,
                    ..
                }) if state.overlay != ControlOverlay::None => {
                    state.overlay = ControlOverlay::None;
                    state.logcat.overlay = LogOverlay::None;
                }
                Event::Key(KeyEvent {
                    code: KeyCode::Up,
                    kind: KeyEventKind::Press | KeyEventKind::Repeat,
                    ..
                }) if state.overlay == ControlOverlay::Palette => {
                    state.overlay_selected = state.overlay_selected.saturating_sub(1);
                }
                Event::Key(KeyEvent {
                    code: KeyCode::Down,
                    kind: KeyEventKind::Press | KeyEventKind::Repeat,
                    ..
                }) if state.overlay == ControlOverlay::Palette => {
                    let count = fuzzy_actions(&state.overlay_query).len();
                    state.overlay_selected =
                        (state.overlay_selected + 1).min(count.saturating_sub(1));
                }
                Event::Key(KeyEvent {
                    code: KeyCode::Enter,
                    kind: KeyEventKind::Press,
                    ..
                }) if state.overlay == ControlOverlay::Palette => {
                    if let Some(item) =
                        fuzzy_actions(&state.overlay_query).get(state.overlay_selected)
                    {
                        let action = item.action;
                        state.overlay = ControlOverlay::None;
                        if apply_named_action(&mut state, action, backend, profile)? {
                            return Ok(());
                        }
                    }
                }
                Event::Key(KeyEvent {
                    code: KeyCode::Enter,
                    kind: KeyEventKind::Press,
                    ..
                }) if state.overlay == ControlOverlay::Search => {
                    state.overlay = ControlOverlay::None;
                }
                Event::Key(KeyEvent {
                    code: KeyCode::Backspace,
                    kind: KeyEventKind::Press | KeyEventKind::Repeat,
                    ..
                }) if matches!(
                    state.overlay,
                    ControlOverlay::Palette | ControlOverlay::Search
                ) =>
                {
                    state.overlay_query.pop();
                    state.overlay_selected = 0;
                    if state.overlay == ControlOverlay::Search {
                        apply_search_query(&mut state);
                    }
                }
                Event::Key(KeyEvent {
                    code: KeyCode::Char(character),
                    kind: KeyEventKind::Press | KeyEventKind::Repeat,
                    ..
                }) if matches!(
                    state.overlay,
                    ControlOverlay::Palette | ControlOverlay::Search
                ) =>
                {
                    state.overlay_query.push(character);
                    state.overlay_selected = 0;
                    if state.overlay == ControlOverlay::Search {
                        apply_search_query(&mut state);
                    }
                }
                Event::Key(_) | Event::Mouse(_) if state.overlay != ControlOverlay::None => {}
                Event::Key(KeyEvent {
                    code: KeyCode::Tab,
                    kind: KeyEventKind::Press,
                    ..
                }) => state.focus = state.focus.next(),
                Event::Key(KeyEvent {
                    code: KeyCode::BackTab,
                    kind: KeyEventKind::Press,
                    ..
                }) => state.focus = state.focus.previous(),
                Event::Key(KeyEvent {
                    code: KeyCode::Char(number @ '1'..='7'),
                    modifiers: KeyModifiers::NONE,
                    kind: KeyEventKind::Press | KeyEventKind::Repeat,
                    ..
                }) => {
                    if let Some(workspace) = WorkspaceId::from_number(number) {
                        activate_workspace(&mut state, workspace, backend, profile)?;
                    }
                }
                Event::Key(KeyEvent {
                    code: KeyCode::Left,
                    kind: KeyEventKind::Press | KeyEventKind::Repeat,
                    ..
                }) if state.focus == FocusRegion::WorkspaceBar => {
                    let workspace = adjacent_workspace(state.active_workspace, false);
                    activate_workspace(&mut state, workspace, backend, profile)?;
                }
                Event::Key(KeyEvent {
                    code: KeyCode::Right,
                    kind: KeyEventKind::Press | KeyEventKind::Repeat,
                    ..
                }) if state.focus == FocusRegion::WorkspaceBar => {
                    let workspace = adjacent_workspace(state.active_workspace, true);
                    activate_workspace(&mut state, workspace, backend, profile)?;
                }
                Event::Key(KeyEvent {
                    code: KeyCode::Left | KeyCode::Right,
                    kind: KeyEventKind::Press | KeyEventKind::Repeat,
                    ..
                }) if matches!(
                    state.active_workspace,
                    WorkspaceId::Devices | WorkspaceId::Tasks
                ) =>
                {
                    state.tooling.move_subview(true);
                }
                Event::Key(KeyEvent {
                    code: KeyCode::Up,
                    kind: KeyEventKind::Press | KeyEventKind::Repeat,
                    ..
                }) => move_workspace_selection(&mut state, false),
                Event::Key(KeyEvent {
                    code: KeyCode::Down,
                    kind: KeyEventKind::Press | KeyEventKind::Repeat,
                    ..
                }) => move_workspace_selection(&mut state, true),
                Event::Key(KeyEvent {
                    code: KeyCode::Enter,
                    kind: KeyEventKind::Press,
                    ..
                }) => {
                    activate_current_selection(&mut state);
                }
                Event::Key(KeyEvent {
                    code: KeyCode::Char(key),
                    kind: KeyEventKind::Press | KeyEventKind::Repeat,
                    ..
                }) if state.active_workspace == WorkspaceId::Tests => {
                    state.input_count = state.input_count.saturating_add(1);
                    let action = state.tests.handle_key(key);
                    if action != crate::TestWorkspaceAction::None {
                        state.notice = "This test action is not connected in this build.".into();
                    }
                }
                Event::Key(KeyEvent {
                    code: KeyCode::Char(key),
                    kind: KeyEventKind::Press | KeyEventKind::Repeat,
                    ..
                }) if matches!(
                    state.active_workspace,
                    WorkspaceId::Devices | WorkspaceId::Tasks | WorkspaceId::Doctor
                ) =>
                {
                    state.input_count = state.input_count.saturating_add(1);
                    let action = state.tooling.handle_key(key);
                    if action != crate::ToolingAction::None {
                        state.notice = "This tooling action is not connected in this build.".into();
                    }
                }
                Event::Key(KeyEvent {
                    code: KeyCode::Char(key),
                    kind: KeyEventKind::Press | KeyEventKind::Repeat,
                    ..
                }) if state.active_workspace == WorkspaceId::Run => {
                    state.input_count = state.input_count.saturating_add(1);
                    let action = state.run.handle_key(key);
                    if action != crate::RunWorkspaceAction::None {
                        state.notice = "This run action is not connected in this build.".into();
                    }
                }
                Event::Key(
                    key @ KeyEvent {
                        kind: KeyEventKind::Press | KeyEventKind::Repeat,
                        ..
                    },
                ) if state.active_workspace == WorkspaceId::Logcat => {
                    state.input_count = state.input_count.saturating_add(1);
                    let action = state.logcat.handle_key(key);
                    handle_logcat_action(&mut state, action, backend);
                }
                Event::Key(KeyEvent {
                    kind: KeyEventKind::Press | KeyEventKind::Repeat,
                    ..
                }) => state.input_count = state.input_count.saturating_add(1),
                Event::Mouse(mouse) => handle_mouse(&mut state, mouse, backend, profile)?,
                _ => {}
            }
        }
        let workspace_dirty = match state.active_workspace {
            WorkspaceId::Overview => state.overview.dirty,
            WorkspaceId::Run => state.run.dirty,
            WorkspaceId::Tests => state.tests.dirty,
            WorkspaceId::Logcat => state.logcat.dirty,
            WorkspaceId::Devices | WorkspaceId::Tasks | WorkspaceId::Doctor => state.tooling.dirty,
        };
        let animation_dirty = state.animation.active();
        if last_draw.elapsed() >= profile.frame_interval()
            && (state.dirty || workspace_dirty || animation_dirty)
        {
            terminal.draw(|frame| render(frame, &mut state, theme))?;
            state.dirty = false;
            let _ = state.animation.advance();
            last_draw = Instant::now();
        }
    }
}

fn request_exit(state: &mut ShellState, backend: Option<&dyn LogcatBackend>) -> bool {
    let active_jobs = backend.map_or(0, LogcatBackend::active_foreground_jobs);
    if active_jobs == 0 {
        true
    } else {
        state.exit_prompt_jobs = Some(active_jobs);
        false
    }
}

fn activate_workspace(
    state: &mut ShellState,
    workspace: WorkspaceId,
    backend: &mut Option<Box<dyn LogcatBackend>>,
    profile: TerminalProfile,
) -> Result<(), ShellError> {
    state.active_workspace = workspace;
    state.notice.clear();
    match workspace {
        WorkspaceId::Devices
            if !matches!(
                state.tooling.tab,
                ToolingTab::Devices | ToolingTab::Emulators
            ) =>
        {
            state.tooling.set_tab(ToolingTab::Devices);
        }
        WorkspaceId::Tasks
            if !matches!(
                state.tooling.tab,
                ToolingTab::GradleTasks | ToolingTab::Commands
            ) =>
        {
            state.tooling.set_tab(ToolingTab::GradleTasks);
        }
        WorkspaceId::Doctor => state.tooling.set_tab(ToolingTab::Doctor),
        WorkspaceId::Logcat if !state.logcat_started => {
            state.logcat_started = true;
            match backend.as_deref_mut() {
                Some(backend) => backend.start().map_err(ShellError::Backend)?,
                None => state
                    .logcat
                    .set_status("Select a module, variant, package, and device to start Logcat."),
            }
        }
        _ => {}
    }
    state.animation.start(6, profile.reduced_motion);
    state.dirty = true;
    Ok(())
}

fn adjacent_workspace(current: WorkspaceId, forward: bool) -> WorkspaceId {
    let index = WorkspaceId::ALL
        .iter()
        .position(|workspace| *workspace == current)
        .unwrap_or_default();
    let next = if forward {
        (index + 1) % WorkspaceId::ALL.len()
    } else {
        index.checked_sub(1).unwrap_or(WorkspaceId::ALL.len() - 1)
    };
    WorkspaceId::ALL[next]
}

fn move_workspace_selection(state: &mut ShellState, down: bool) {
    match state.active_workspace {
        WorkspaceId::Overview => state.overview.move_selection(down),
        WorkspaceId::Run => {
            let _ = state.run.handle_key(if down { 'j' } else { 'k' });
        }
        WorkspaceId::Tests => {
            let _ = state.tests.handle_key(if down { 'j' } else { 'k' });
        }
        WorkspaceId::Logcat => {
            let code = if down { KeyCode::Down } else { KeyCode::Up };
            let _ = state
                .logcat
                .handle_key(KeyEvent::new(code, KeyModifiers::NONE));
        }
        WorkspaceId::Devices | WorkspaceId::Tasks | WorkspaceId::Doctor => {
            let _ = state.tooling.handle_key(if down { 'j' } else { 'k' });
        }
    }
}

fn activate_current_selection(state: &mut ShellState) {
    state.notice = match state.active_workspace {
        WorkspaceId::Overview => "This action is not connected in this build.",
        WorkspaceId::Run => "Run controls are not connected in this build.",
        WorkspaceId::Tests => "Test controls are not connected in this build.",
        WorkspaceId::Logcat => "Use Space to pause or Ctrl+P for Logcat actions.",
        WorkspaceId::Devices | WorkspaceId::Tasks | WorkspaceId::Doctor => {
            "Tooling controls are not connected in this build."
        }
    }
    .into();
}

fn apply_search_query(state: &mut ShellState) {
    match state.active_workspace {
        WorkspaceId::Logcat => {
            if let Err(error) = state.logcat.set_text_search(&state.overlay_query) {
                state.logcat.set_status(format!("Invalid search: {error}"));
            }
        }
        WorkspaceId::Devices | WorkspaceId::Tasks => {
            state.tooling.set_search(state.overlay_query.clone());
        }
        _ => {}
    }
}

fn apply_named_action(
    state: &mut ShellState,
    action: NamedAction,
    backend: &mut Option<Box<dyn LogcatBackend>>,
    profile: TerminalProfile,
) -> Result<bool, ShellError> {
    let workspace = match action {
        NamedAction::Overview => Some(WorkspaceId::Overview),
        NamedAction::Run => Some(WorkspaceId::Run),
        NamedAction::Tests => Some(WorkspaceId::Tests),
        NamedAction::Logcat => Some(WorkspaceId::Logcat),
        NamedAction::Devices => Some(WorkspaceId::Devices),
        NamedAction::Tasks => Some(WorkspaceId::Tasks),
        NamedAction::Doctor => Some(WorkspaceId::Doctor),
        _ => None,
    };
    if let Some(workspace) = workspace {
        activate_workspace(state, workspace, backend, profile)?;
        return Ok(false);
    }
    match action {
        NamedAction::Help => state.overlay = ControlOverlay::Help,
        NamedAction::Search => state.overlay = ControlOverlay::Search,
        NamedAction::FocusNext => state.focus = state.focus.next(),
        NamedAction::FocusPrevious => state.focus = state.focus.previous(),
        NamedAction::Quit => return Ok(request_exit(state, backend.as_deref())),
        NamedAction::CommandPalette => state.overlay = ControlOverlay::Palette,
        _ => {}
    }
    Ok(false)
}

fn handle_logcat_action(
    state: &mut ShellState,
    action: LogWorkspaceAction,
    backend: &mut Option<Box<dyn LogcatBackend>>,
) {
    let Some(backend) = backend.as_deref_mut() else {
        if action != LogWorkspaceAction::None {
            state
                .logcat
                .set_status("Logcat is not connected in this build.");
        }
        return;
    };
    let result = match action {
        LogWorkspaceAction::None => Ok(()),
        LogWorkspaceAction::ScopeChanged(device) => backend.set_device_scope(device),
        LogWorkspaceAction::ProcessSelectionRequested => backend.select_process(),
        LogWorkspaceAction::CopyLine => backend.copy(false),
        LogWorkspaceAction::CopyGroup => backend.copy(true),
        LogWorkspaceAction::ExportRequested => backend.export(),
        LogWorkspaceAction::RecordingToggled => backend.toggle_recording(),
    };
    if let Err(error) = result {
        state.logcat.set_status(error);
    }
}

fn handle_mouse(
    state: &mut ShellState,
    mouse: crossterm::event::MouseEvent,
    backend: &mut Option<Box<dyn LogcatBackend>>,
    profile: TerminalProfile,
) -> Result<(), ShellError> {
    state.input_count = state.input_count.saturating_add(1);
    if mouse.kind == MouseEventKind::Down(MouseButton::Left)
        && let Some(workspace) = state
            .hit_regions
            .iter()
            .find(|region| point_in_rect(mouse.column, mouse.row, region.area))
            .map(|region| region.workspace)
    {
        activate_workspace(state, workspace, backend, profile)?;
        return Ok(());
    }
    if mouse.kind == MouseEventKind::Down(MouseButton::Left) {
        let handled = match state.active_workspace {
            WorkspaceId::Overview => state.overview.handle_click(mouse.column, mouse.row),
            WorkspaceId::Run => state.run.handle_click(mouse.column, mouse.row),
            WorkspaceId::Tests => state.tests.handle_click(mouse.column, mouse.row),
            WorkspaceId::Devices | WorkspaceId::Tasks | WorkspaceId::Doctor => {
                state.tooling.handle_click(mouse.column, mouse.row)
            }
            WorkspaceId::Logcat => false,
        };
        if handled {
            return Ok(());
        }
    }
    match mouse.kind {
        MouseEventKind::ScrollUp => move_workspace_selection(state, false),
        MouseEventKind::ScrollDown => move_workspace_selection(state, true),
        _ => {}
    }
    Ok(())
}

fn point_in_rect(x: u16, y: u16, area: Rect) -> bool {
    x >= area.x
        && x < area.x.saturating_add(area.width)
        && y >= area.y
        && y < area.y.saturating_add(area.height)
}

fn drain_logcat_notifications(state: &mut ShellState, backend: &mut dyn LogcatBackend) -> usize {
    let mut drained = 0;
    for _ in 0..MAX_LOG_NOTIFICATIONS_PER_TICK {
        let Some(notification) = backend.try_recv() else {
            break;
        };
        drained += 1;
        match notification {
            LogcatBackendEvent::Records(records) => state.logcat.ingest(records),
            LogcatBackendEvent::Status(status) => state.logcat.set_status(status),
            LogcatBackendEvent::Error(error) => state
                .logcat
                .set_status(format!("Logcat unavailable: {error}")),
            LogcatBackendEvent::Recording(active) => {
                state.logcat.recording = active;
                state.logcat.dirty = true;
            }
            LogcatBackendEvent::Exporting(active) => {
                state.logcat.exporting = active;
                state.logcat.dirty = true;
            }
        }
    }
    drained
}

fn drain_project_notifications(state: &mut ShellState, backend: &mut dyn ProjectBackend) -> usize {
    let mut drained = 0;
    for _ in 0..MAX_PROJECT_NOTIFICATIONS_PER_TICK {
        let Some(notification) = backend.try_recv() else {
            break;
        };
        drained += 1;
        apply_project_event(state, notification);
    }
    drained
}

fn apply_project_event(state: &mut ShellState, event: ProjectBackendEvent) {
    match event {
        ProjectBackendEvent::Detected {
            root,
            wrapper_found,
        } => {
            set_project_identity(state, &root);
            state.project_phase = ProjectPhase::Detected;
            state.overview.model_status = if wrapper_found {
                "Project detected · Loading project model".into()
            } else {
                "Project detected · Gradle wrapper missing".into()
            };
            state.run.model_status = state.overview.model_status.clone();
        }
        ProjectBackendEvent::Model(snapshot) => {
            let snapshot = *snapshot;
            set_project_identity(state, &snapshot.project.root);
            state.project_phase = ProjectPhase::Available;
            let module_count = snapshot.project.modules.len();
            state.tooling.tasks = snapshot.project.tasks;
            state.overview.model_status = project_model_status(
                snapshot.freshness,
                snapshot.support,
                snapshot.degraded_reason.as_ref(),
                module_count,
            );
            state.run.model_status = state.overview.model_status.clone();
        }
        ProjectBackendEvent::Error { root, message } => {
            let model_usable = state.project_phase == ProjectPhase::Available && root.is_some();
            if let Some(root) = root {
                set_project_identity(state, &root);
                state.overview.model_status = if model_usable {
                    format!("Project refresh unavailable · Using cached model: {message}")
                } else {
                    format!("Project model unavailable: {message}")
                };
            } else {
                state.project_name = None;
                state.overview.project = None;
                state.run.project = None;
                state.overview.model_status = format!("No Android Gradle project: {message}");
            }
            state.project_phase = if model_usable {
                ProjectPhase::Available
            } else {
                ProjectPhase::Unavailable
            };
            state.run.model_status = state.overview.model_status.clone();
            if !model_usable {
                state.tooling.tasks.clear();
            }
        }
    }
    state.overview.dirty = true;
    state.run.dirty = true;
    state.tooling.dirty = true;
    state.dirty = true;
}

fn set_project_identity(state: &mut ShellState, root: &std::path::Path) {
    let name = root
        .file_name()
        .filter(|name| !name.is_empty())
        .map_or_else(
            || root.display().to_string(),
            |name| name.to_string_lossy().into_owned(),
        );
    state.project_name = Some(name.clone());
    state.overview.project = Some(root.display().to_string());
    state.run.project = Some(name);
}

fn project_model_status(
    freshness: ModelFreshness,
    support: ProjectSupport,
    degraded_reason: Option<&DegradedReason>,
    module_count: usize,
) -> String {
    let support_label = match support {
        ProjectSupport::Full => "Full support",
        ProjectSupport::Degraded => "Degraded support",
        ProjectSupport::Unsupported => "Unsupported project",
    };
    let reason = degraded_reason.map(format_degraded_reason);
    match freshness {
        ModelFreshness::Current if support == ProjectSupport::Full => {
            format!("Model current · {support_label} · {module_count} modules")
        }
        ModelFreshness::Current => format!(
            "Model current · {support_label}: {}",
            reason.unwrap_or_else(|| "limited Gradle capabilities".into())
        ),
        ModelFreshness::Stale | ModelFreshness::Provisional => {
            format!("Using stale model · Refreshing · {support_label} · {module_count} modules")
        }
        ModelFreshness::Refreshing => {
            format!("Refreshing project model · {module_count} modules")
        }
        ModelFreshness::Degraded => {
            let reason = reason.unwrap_or_else(|| "unknown model failure".into());
            format!("Model degraded · Using cached model: {reason}")
        }
    }
}

fn format_degraded_reason(reason: &DegradedReason) -> String {
    match reason {
        DegradedReason::UnsupportedAgp {
            detected,
            supported,
        } => format!("AGP {detected}; supported {supported}"),
        DegradedReason::IncompatibleProtocol { expected, found } => {
            format!("bridge protocol {found}; expected {expected}")
        }
        DegradedReason::ApiUnavailable { api } => format!("required API unavailable: {api}"),
        DegradedReason::MissingWrapper => "Gradle wrapper missing".into(),
        DegradedReason::ConfigurationFailed { message }
        | DegradedReason::CacheInvalid { message } => message.clone(),
        DegradedReason::BridgeFailed { code, message } => format!("{code}: {message}"),
    }
}

fn should_exit(event: &Event) -> bool {
    matches!(
        event,
        Event::Key(KeyEvent {
            code: KeyCode::Char('q'),
            kind: KeyEventKind::Press,
            ..
        }) | Event::Key(KeyEvent {
            code: KeyCode::Char('c'),
            modifiers: KeyModifiers::CONTROL,
            kind: KeyEventKind::Press,
            ..
        })
    )
}

fn render(frame: &mut Frame<'_>, state: &mut ShellState, theme: LazuliTheme) {
    let area = frame.area();
    frame.render_widget(Block::new().style(theme.canvas()), area);
    let layout_mode = DashboardLayout::for_size(area.width, area.height);
    if layout_mode == DashboardLayout::ResizeWarning {
        render_resize_message(frame, area, theme);
        return;
    }

    let rows = Layout::vertical([
        Constraint::Length(if layout_mode == DashboardLayout::SingleWorkspace {
            1
        } else {
            2
        }),
        Constraint::Length(2),
        Constraint::Min(3),
        Constraint::Length(1),
    ])
    .split(area);

    let header_area = inset_horizontal(rows[0], 2);
    let project = state
        .project_name
        .as_deref()
        .unwrap_or(match state.project_phase {
            ProjectPhase::Detecting => "Detecting project",
            ProjectPhase::Detected => "Project detected",
            ProjectPhase::Available => "Project available",
            ProjectPhase::Unavailable => "No project",
        });
    let target = match (&state.run.module, &state.run.variant) {
        (Some(module), Some(variant)) => format!("{module} / {variant}"),
        _ => "No target selected".into(),
    };
    let device = state.run.device.as_deref().unwrap_or("No device");
    let context_separator = if theme.glyphs == GlyphMode::Ascii {
        "|"
    } else {
        "·"
    };
    let mut header = vec![Line::from(vec![
        Span::styled("DexDeck", theme.accent()),
        Span::styled(format!("   {project}"), theme.muted()),
    ])];
    if layout_mode != DashboardLayout::SingleWorkspace {
        header.push(Line::styled(
            format!("{target}   {context_separator}   {device}"),
            theme.muted(),
        ));
    }
    frame.render_widget(Paragraph::new(header), header_area);

    state.hit_regions.clear();
    let tabs_area = inset_horizontal(Rect::new(rows[1].x, rows[1].y, rows[1].width, 1), 2);
    let tabs = if layout_mode == DashboardLayout::SingleWorkspace {
        Line::from(vec![
            Span::styled(state.active_workspace.label(), theme.accent()),
            Span::styled("   Ctrl+P Commands", theme.muted()),
        ])
    } else {
        let mut spans = Vec::new();
        let mut x = tabs_area.x;
        for workspace in WorkspaceId::ALL {
            let label = format!("{} {}  ", workspace.number(), workspace.label());
            let width = u16::try_from(label.len()).unwrap_or(u16::MAX);
            state.hit_regions.push(HitRegion {
                area: Rect::new(x, tabs_area.y, width, 1),
                workspace,
            });
            x = x.saturating_add(width);
            spans.push(Span::styled(
                label,
                if workspace == state.active_workspace {
                    theme.accent().add_modifier(Modifier::UNDERLINED)
                } else {
                    theme.muted()
                },
            ));
        }
        Line::from(spans)
    };
    frame.render_widget(Paragraph::new(tabs), tabs_area);
    let separator_area = inset_horizontal(
        Rect::new(rows[1].x, rows[1].y.saturating_add(1), rows[1].width, 1),
        2,
    );
    let separator = if theme.glyphs == GlyphMode::Ascii {
        '-'
    } else {
        '─'
    };
    frame.render_widget(
        Paragraph::new(
            separator
                .to_string()
                .repeat(usize::from(separator_area.width)),
        )
        .style(theme.separator()),
        separator_area,
    );

    let content = content_area(rows[2]);
    match state.active_workspace {
        WorkspaceId::Overview => state.overview.render(frame, content, layout_mode, theme),
        WorkspaceId::Run => state.run.render(frame, content, theme),
        WorkspaceId::Tests => state.tests.render(frame, content, theme),
        WorkspaceId::Logcat => state.logcat.render(frame, content, theme),
        WorkspaceId::Devices | WorkspaceId::Tasks | WorkspaceId::Doctor => {
            state.tooling.render(frame, content, theme);
        }
    }

    let footer_text = if state.notice.is_empty() {
        state.overview.model_status.as_str()
    } else {
        state.notice.as_str()
    };
    let hints = if area.width >= 100 && theme.glyphs == GlyphMode::Unicode {
        "Tab Focus   ↑↓ Select   Enter Choose   ? Shortcuts   q Quit"
    } else if area.width >= 100 {
        "Tab Focus   Up/Down Select   Enter Choose   ? Shortcuts   q Quit"
    } else if area.width >= 70 {
        "Tab Focus   Enter Choose   ? Help   q Quit"
    } else {
        "? Help   q Quit"
    };
    let available = usize::from(area.width.saturating_sub(4));
    let gap = available
        .saturating_sub(footer_text.len() + hints.len())
        .max(2);
    frame.render_widget(
        Paragraph::new(format!("{footer_text}{}{hints}", " ".repeat(gap))).style(theme.muted()),
        inset_horizontal(rows[3], 2),
    );
    render_control_overlay(frame, state, theme);
    render_exit_prompt(frame, state, theme);
}

fn inset_horizontal(area: Rect, amount: u16) -> Rect {
    Rect::new(
        area.x.saturating_add(amount),
        area.y,
        area.width.saturating_sub(amount.saturating_mul(2)),
        area.height,
    )
}

fn content_area(area: Rect) -> Rect {
    let horizontal = inset_horizontal(area, 2);
    Rect::new(
        horizontal.x,
        horizontal.y.saturating_add(1),
        horizontal.width,
        horizontal.height.saturating_sub(1),
    )
}

fn render_exit_prompt(frame: &mut Frame<'_>, state: &ShellState, theme: LazuliTheme) {
    let Some(job_count) = state.exit_prompt_jobs else {
        return;
    };
    let area = frame.area();
    let width = area.width.saturating_sub(4).min(70);
    let prompt = Rect::new(
        area.x + area.width.saturating_sub(width) / 2,
        area.y + area.height.saturating_sub(5) / 2,
        width,
        5,
    );
    frame.render_widget(Clear, prompt);
    frame.render_widget(
        Paragraph::new(format!(
            "{job_count} foreground job(s) are active.\ny/q detach and exit | c cancel owned jobs and exit | n stay"
        ))
        .style(
            Style::default()
                .fg(theme.colors.warning)
                .bg(theme.colors.surface),
        )
        .block(panel(" Active jobs ", theme)),
        prompt,
    );
}

fn render_control_overlay(frame: &mut Frame<'_>, state: &ShellState, theme: LazuliTheme) {
    if state.overlay == ControlOverlay::None {
        return;
    }
    let area = frame.area();
    let width = area.width.saturating_sub(4).min(72);
    let height = area.height.saturating_sub(4).min(16);
    let overlay = Rect::new(
        area.x + area.width.saturating_sub(width) / 2,
        area.y + area.height.saturating_sub(height) / 2,
        width,
        height,
    );
    let (title, content) = match state.overlay {
        ControlOverlay::Palette => {
            let matches = fuzzy_actions(&state.overlay_query);
            let rows = matches
                .iter()
                .enumerate()
                .take(height.saturating_sub(3).into())
                .map(|(index, item)| {
                    Line::styled(
                        format!(
                            "{} {}",
                            if index == state.overlay_selected {
                                ">"
                            } else {
                                " "
                            },
                            item.action.label()
                        ),
                        if index == state.overlay_selected {
                            theme.selected()
                        } else {
                            theme.muted()
                        },
                    )
                })
                .collect::<Vec<_>>();
            let mut content = vec![Line::from(vec![
                Span::styled("> ", theme.accent()),
                Span::raw(&state.overlay_query),
            ])];
            content.extend(rows);
            (" Commands ", content)
        }
        ControlOverlay::Help => (
            " Help ",
            vec![
                Line::raw(if theme.glyphs == GlyphMode::Ascii {
                    "1-7 workspaces      Tab / Shift+Tab focus"
                } else {
                    "1–7 workspaces      Tab / Shift+Tab focus"
                }),
                Line::raw(if theme.glyphs == GlyphMode::Ascii {
                    "Up/Down select      Left/Right change view"
                } else {
                    "↑↓ select           ←→ change view"
                }),
                Line::raw("Enter open          / search"),
                Line::raw("Ctrl+P commands     ? help"),
                Line::raw("Esc closes overlays; q or Ctrl+C exits"),
            ],
        ),
        ControlOverlay::Search => (
            " Search ",
            vec![Line::from(vec![
                Span::styled("> ", theme.accent()),
                Span::raw(&state.overlay_query),
            ])],
        ),
        ControlOverlay::None => return,
    };
    frame.render_widget(Clear, overlay);
    frame.render_widget(
        Paragraph::new(content)
            .style(
                Style::default()
                    .fg(theme.colors.text_primary)
                    .bg(theme.colors.surface),
            )
            .block(panel(title, theme)),
        overlay,
    );
}

fn render_resize_message(frame: &mut Frame<'_>, area: Rect, theme: LazuliTheme) {
    let message = format!(
        "Terminal too small: {}x{}. Resize to at least {MINIMUM_WIDTH}x{MINIMUM_HEIGHT}.",
        area.width, area.height
    );
    frame.render_widget(
        Paragraph::new(message).alignment(Alignment::Center).style(
            Style::default()
                .fg(theme.colors.warning)
                .add_modifier(Modifier::BOLD),
        ),
        vertically_centered(area),
    );
}

fn vertically_centered(area: Rect) -> Rect {
    let top = area.height.saturating_sub(1) / 2;
    Rect::new(area.x, area.y.saturating_add(top), area.width, 1)
}

fn panel(title: &'static str, theme: LazuliTheme) -> Block<'static> {
    Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_set(if theme.glyphs == GlyphMode::Ascii {
            ASCII_BORDER
        } else {
            border::ROUNDED
        })
        .border_style(Style::default().fg(theme.colors.border))
        .style(Style::default().bg(theme.colors.surface))
}

struct TerminalSession {
    terminal: Terminal<CrosstermBackend<Stdout>>,
    previous_panic_hook: Arc<dyn Fn(&panic::PanicHookInfo<'_>) + Send + Sync + 'static>,
}

impl TerminalSession {
    fn enter() -> Result<Self, ShellError> {
        enable_raw_mode()?;
        #[cfg(debug_assertions)]
        if std::env::var_os("DEXDECK_INTERNAL_TEST_FAIL_AFTER_RAW").is_some() {
            disable_raw_mode()?;
            return Err(ShellError::Io(io::Error::other(
                "injected terminal initialization failure",
            )));
        }
        let mut stdout = io::stdout();
        if let Err(error) = execute!(stdout, EnterAlternateScreen, EnableMouseCapture, Hide) {
            let _ = disable_raw_mode();
            return Err(ShellError::Io(error));
        }
        let terminal = match Terminal::new(CrosstermBackend::new(stdout)) {
            Ok(terminal) => terminal,
            Err(error) => {
                restore_terminal();
                return Err(ShellError::Io(error));
            }
        };

        let previous_panic_hook: Arc<dyn Fn(&panic::PanicHookInfo<'_>) + Send + Sync + 'static> =
            panic::take_hook().into();
        panic::set_hook(Box::new(move |info| {
            restore_terminal();
            let location = info.location().map_or_else(
                || "unknown location".to_owned(),
                |location| format!("{}:{}", location.file(), location.line()),
            );
            eprintln!("dexdeck: unexpected panic at {location}; terminal restored");
        }));
        Ok(Self {
            terminal,
            previous_panic_hook,
        })
    }

    const fn terminal_mut(&mut self) -> &mut Terminal<CrosstermBackend<Stdout>> {
        &mut self.terminal
    }
}

impl Drop for TerminalSession {
    fn drop(&mut self) {
        let _ = self.terminal.show_cursor();
        restore_terminal();
        let _ = panic::take_hook();
        let previous = Arc::clone(&self.previous_panic_hook);
        panic::set_hook(Box::new(move |info| previous(info)));
    }
}

fn restore_terminal() {
    let _ = disable_raw_mode();
    let mut stdout = io::stdout();
    let _ = execute!(stdout, Show, DisableMouseCapture, LeaveAlternateScreen);
    let _ = stdout.flush();
}

#[derive(Debug, thiserror::Error)]
pub enum ShellError {
    #[error("terminal operation failed: {0}")]
    Io(#[from] io::Error),
    #[error("backend operation failed: {0}")]
    Backend(String),
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;

    use dexdeck_protocol::{BuildInfo, GradleTask, ProjectModel, TaskKind};
    use ratatui::{Terminal, backend::TestBackend, buffer::Buffer};

    use super::*;

    #[test]
    fn minimum_size_renders_only_resize_guidance() -> Result<(), Box<dyn std::error::Error>> {
        let backend = TestBackend::new(39, 3);
        let mut terminal = Terminal::new(backend)?;
        let theme = LazuliTheme::new(ColorCapability::NoColor, GlyphMode::Ascii);
        let mut state = ShellState::default();
        terminal.draw(|frame| render(frame, &mut state, theme))?;

        let mut expected = Buffer::with_lines([
            "                                       ",
            "Terminal too small: 39x3. Resize to at",
            "                                       ",
        ]);
        expected.set_style(
            Rect::new(0, 1, 39, 1),
            Style::default().add_modifier(Modifier::BOLD),
        );
        terminal.backend().assert_buffer(&expected);
        Ok(())
    }

    #[test]
    fn exit_keys_are_explicit() {
        let key = |code, modifiers| {
            Event::Key(KeyEvent::new_with_kind(
                code,
                modifiers,
                KeyEventKind::Press,
            ))
        };
        assert!(should_exit(&key(KeyCode::Char('q'), KeyModifiers::NONE)));
        assert!(!should_exit(&key(KeyCode::Esc, KeyModifiers::NONE)));
        assert!(should_exit(&key(KeyCode::Char('c'), KeyModifiers::CONTROL)));
        assert!(!should_exit(&key(KeyCode::Char('x'), KeyModifiers::NONE)));
    }

    #[test]
    fn layout_breakpoint_snapshot() {
        assert_eq!(
            [
                DashboardLayout::for_size(140, 40),
                DashboardLayout::for_size(90, 20),
                DashboardLayout::for_size(60, 14),
                DashboardLayout::for_size(39, 9),
            ],
            [
                DashboardLayout::Full,
                DashboardLayout::Compact,
                DashboardLayout::SingleWorkspace,
                DashboardLayout::ResizeWarning,
            ]
        );
    }

    #[test]
    fn event_exit_snapshot() {
        let events = [
            Event::Key(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE)),
            Event::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
            Event::Key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL)),
            Event::Key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::NONE)),
        ];
        assert_eq!(
            events.map(|event| should_exit(&event)),
            [true, false, true, false]
        );
    }

    #[test]
    fn full_layout_is_border_light_and_exposes_workspace_navigation()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut terminal = Terminal::new(TestBackend::new(140, 40))?;
        let theme = LazuliTheme::new(ColorCapability::NoColor, GlyphMode::Ascii);
        let mut state = ShellState::default();
        terminal.draw(|frame| render(frame, &mut state, theme))?;
        let rendered = rendered_text(terminal.backend().buffer());

        assert!(rendered.contains("DexDeck   Detecting project"));
        assert!(rendered.contains("1 Overview"));
        assert!(rendered.contains("7 Doctor"));
        assert!(rendered.contains("Ready for development"));
        assert!(!rendered.contains("+---"));
        for unicode_only in ['·', '↑', '↓', '←', '→', '–', '▌'] {
            assert!(!rendered.contains(unicode_only));
        }
        assert_eq!(state.hit_regions.len(), WorkspaceId::ALL.len());
        Ok(())
    }

    #[test]
    fn single_workspace_layout_collapses_the_tab_bar() -> Result<(), Box<dyn std::error::Error>> {
        let mut terminal = Terminal::new(TestBackend::new(60, 14))?;
        let theme = LazuliTheme::new(ColorCapability::NoColor, GlyphMode::Ascii);
        let mut state = ShellState {
            active_workspace: WorkspaceId::Tests,
            ..ShellState::default()
        };
        terminal.draw(|frame| render(frame, &mut state, theme))?;
        let rendered = rendered_text(terminal.backend().buffer());

        assert!(rendered.contains("Tests   Ctrl+P Commands"));
        assert!(!rendered.contains("1 Overview"));
        assert!(state.hit_regions.is_empty());
        Ok(())
    }

    #[test]
    fn workspace_tab_click_changes_the_active_workspace() -> Result<(), Box<dyn std::error::Error>>
    {
        let mut terminal = Terminal::new(TestBackend::new(140, 40))?;
        let theme = LazuliTheme::new(ColorCapability::NoColor, GlyphMode::Ascii);
        let mut state = ShellState::default();
        terminal.draw(|frame| render(frame, &mut state, theme))?;
        let logcat = state
            .hit_regions
            .iter()
            .find(|region| region.workspace == WorkspaceId::Logcat)
            .copied()
            .ok_or_else(|| std::io::Error::other("Logcat tab was not rendered"))?;
        let mouse = crossterm::event::MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: logcat.area.x,
            row: logcat.area.y,
            modifiers: KeyModifiers::NONE,
        };
        let profile = TerminalProfile {
            remote: false,
            tmux: false,
            reduced_motion: true,
            utf8: true,
        };
        handle_mouse(&mut state, mouse, &mut None, profile)?;

        assert_eq!(state.active_workspace, WorkspaceId::Logcat);
        Ok(())
    }

    fn rendered_text(buffer: &Buffer) -> String {
        buffer
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>()
    }

    fn project_snapshot(
        freshness: ModelFreshness,
        support: ProjectSupport,
        degraded_reason: Option<DegradedReason>,
    ) -> ProjectSnapshot {
        let root = PathBuf::from("/workspace/shop-android");
        ProjectSnapshot {
            freshness,
            support,
            degraded_reason,
            project: ProjectModel {
                root: root.clone(),
                build: BuildInfo {
                    root,
                    gradle_version: "8.13".into(),
                    agp_version: Some("8.8".into()),
                    java_version: Some("17".into()),
                    kotlin_plugin_version: Some("2.1".into()),
                },
                included_builds: vec![],
                modules: vec![],
                tasks: vec![GradleTask {
                    path: ":app:assembleDebug".into(),
                    name: "assembleDebug".into(),
                    group: Some("build".into()),
                    description: None,
                    origin_build: "main".into(),
                    module: Some(":app".into()),
                    variant: Some("debug".into()),
                    kind: TaskKind::Assemble,
                }],
                diagnostics: vec![],
            },
        }
    }

    #[test]
    fn project_events_replace_detection_with_live_model_state()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut state = ShellState::default();
        apply_project_event(
            &mut state,
            ProjectBackendEvent::Detected {
                root: PathBuf::from("/workspace/shop-android"),
                wrapper_found: true,
            },
        );
        assert_eq!(state.project_name.as_deref(), Some("shop-android"));
        assert_eq!(state.project_phase, ProjectPhase::Detected);
        assert!(
            state
                .overview
                .model_status
                .contains("Loading project model")
        );

        apply_project_event(
            &mut state,
            ProjectBackendEvent::Model(Box::new(project_snapshot(
                ModelFreshness::Current,
                ProjectSupport::Full,
                None,
            ))),
        );
        assert_eq!(state.project_phase, ProjectPhase::Available);
        assert!(state.overview.model_status.contains("Model current"));
        assert_eq!(state.tooling.tasks.len(), 1);

        let mut terminal = Terminal::new(TestBackend::new(140, 40))?;
        let theme = LazuliTheme::new(ColorCapability::NoColor, GlyphMode::Ascii);
        terminal.draw(|frame| render(frame, &mut state, theme))?;
        let rendered = rendered_text(terminal.backend().buffer());
        assert!(rendered.contains("DexDeck   shop-android"));
        assert!(!rendered.contains("Detecting project"));
        Ok(())
    }

    #[test]
    fn degraded_and_failed_project_states_never_look_like_detection() {
        let mut state = ShellState::default();
        apply_project_event(
            &mut state,
            ProjectBackendEvent::Model(Box::new(project_snapshot(
                ModelFreshness::Degraded,
                ProjectSupport::Degraded,
                Some(DegradedReason::MissingWrapper),
            ))),
        );
        assert!(
            state
                .overview
                .model_status
                .contains("Gradle wrapper missing")
        );
        assert_eq!(state.tooling.tasks.len(), 1);

        apply_project_event(
            &mut state,
            ProjectBackendEvent::Error {
                root: Some(PathBuf::from("/workspace/shop-android")),
                message: "model watcher stopped".into(),
            },
        );
        assert_eq!(state.project_phase, ProjectPhase::Available);
        assert_eq!(state.tooling.tasks.len(), 1);
        assert!(state.overview.model_status.contains("Using cached model"));

        apply_project_event(
            &mut state,
            ProjectBackendEvent::Error {
                root: None,
                message: "settings.gradle(.kts) was not found".into(),
            },
        );
        assert_eq!(state.project_phase, ProjectPhase::Unavailable);
        assert_eq!(state.project_name, None);
        assert!(
            state
                .overview
                .model_status
                .starts_with("No Android Gradle project")
        );
        assert!(state.tooling.tasks.is_empty());
    }

    #[test]
    fn drains_a_fixed_number_of_project_notifications_per_tick() {
        struct Backend(VecDeque<ProjectBackendEvent>);
        impl ProjectBackend for Backend {
            fn start(&mut self) -> Result<(), String> {
                Ok(())
            }
            fn try_recv(&mut self) -> Option<ProjectBackendEvent> {
                self.0.pop_front()
            }
            fn stop(&mut self) {}
        }
        let mut backend = Backend(
            (0..10)
                .map(|index| ProjectBackendEvent::Detected {
                    root: PathBuf::from(format!("/workspace/project-{index}")),
                    wrapper_found: true,
                })
                .collect(),
        );
        let mut state = ShellState::default();
        assert_eq!(drain_project_notifications(&mut state, &mut backend), 4);
        assert_eq!(backend.0.len(), 6);
    }

    #[test]
    fn drains_a_fixed_number_of_log_notifications_per_tick() {
        struct Backend(VecDeque<LogcatBackendEvent>);
        impl LogcatBackend for Backend {
            fn start(&mut self) -> Result<(), String> {
                Ok(())
            }
            fn try_recv(&mut self) -> Option<LogcatBackendEvent> {
                self.0.pop_front()
            }
            fn set_device_scope(&mut self, _: bool) -> Result<(), String> {
                Ok(())
            }
            fn select_process(&mut self) -> Result<(), String> {
                Ok(())
            }
            fn copy(&mut self, _: bool) -> Result<(), String> {
                Ok(())
            }
            fn export(&mut self) -> Result<(), String> {
                Ok(())
            }
            fn toggle_recording(&mut self) -> Result<(), String> {
                Ok(())
            }
            fn stop(&mut self) {}
        }
        let mut backend = Backend(
            (0..20)
                .map(|index| LogcatBackendEvent::Status(format!("status {index}")))
                .collect(),
        );
        let mut state = ShellState::default();
        assert_eq!(drain_logcat_notifications(&mut state, &mut backend), 8);
        assert_eq!(backend.0.len(), 12);
    }
}
