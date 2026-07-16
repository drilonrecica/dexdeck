use std::{
    io::{self, Stdout, Write},
    panic,
    sync::Arc,
    time::{Duration, Instant},
};

use crossterm::{
    cursor::{Hide, Show},
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyEventKind,
        KeyModifiers,
    },
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Frame, Terminal,
    backend::{Backend, CrosstermBackend},
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    symbols::border,
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

use dexdeck_protocol::LogRecord;

use crate::{
    ColorCapability, GlyphMode, LazuliTheme, LogOverlay, LogWorkspaceAction, LogcatWorkspace,
    TestWorkspace,
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

const FRAME_INTERVAL: Duration = Duration::from_millis(34);
const MAX_LOG_NOTIFICATIONS_PER_TICK: usize = 8;

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
    logcat_active: bool,
    tests_active: bool,
    logcat_started: bool,
    logcat: LogcatWorkspace,
    tests: TestWorkspace,
}

pub fn run(options: ShellOptions) -> Result<(), ShellError> {
    run_with_logcat(options, None)
}

pub fn run_with_logcat(
    options: ShellOptions,
    mut logcat: Option<Box<dyn LogcatBackend>>,
) -> Result<(), ShellError> {
    let theme = LazuliTheme::new(
        ColorCapability::detect(options.no_color),
        if options.ascii {
            GlyphMode::Ascii
        } else {
            GlyphMode::Unicode
        },
    );
    let mut session = TerminalSession::enter()?;
    #[cfg(debug_assertions)]
    if std::env::var_os("DEXDECK_INTERNAL_TEST_PANIC_AFTER_ENTER").is_some() {
        panic!("injected terminal restoration test panic");
    }
    let result = run_loop(session.terminal_mut(), theme, &mut logcat);
    if let Some(logcat) = &mut logcat {
        logcat.stop();
    }
    result
}

fn run_loop<B: Backend>(
    terminal: &mut Terminal<B>,
    theme: LazuliTheme,
    backend: &mut Option<Box<dyn LogcatBackend>>,
) -> Result<(), ShellError> {
    let mut state = ShellState::default();
    terminal.draw(|frame| render(frame, &mut state, theme))?;
    let mut last_draw = Instant::now();
    loop {
        if state.logcat_active
            && let Some(backend) = backend.as_deref_mut()
        {
            drain_logcat_notifications(&mut state, backend);
        }
        if event::poll(Duration::from_millis(16))? {
            let event = event::read()?;
            let overlay_open = state.logcat_active && state.logcat.overlay != LogOverlay::None;
            if should_exit(&event) && !overlay_open {
                return Ok(());
            }
            match event {
                Event::Resize(width, height) => {
                    state.last_size = Some((width, height));
                    state.logcat.dirty = true;
                }
                Event::Key(KeyEvent {
                    code: KeyCode::Char('l'),
                    modifiers: KeyModifiers::CONTROL,
                    kind: KeyEventKind::Press,
                    ..
                }) => {
                    state.logcat_active = true;
                    state.tests_active = false;
                    if !state.logcat_started {
                        state.logcat_started = true;
                        match backend.as_deref_mut() {
                            Some(backend) => {
                                if let Err(error) = backend.start() {
                                    state
                                        .logcat
                                        .set_status(format!("Cannot start Logcat: {error}"));
                                }
                            }
                            None => state.logcat.set_status(
                                "Select a module, variant, package, and device to start Logcat.",
                            ),
                        }
                    }
                }
                Event::Key(KeyEvent {
                    code: KeyCode::Char('t'),
                    modifiers: KeyModifiers::CONTROL,
                    kind: KeyEventKind::Press,
                    ..
                }) => {
                    state.tests_active = true;
                    state.logcat_active = false;
                    state.tests.dirty = true;
                }
                Event::Key(KeyEvent {
                    code: KeyCode::Char(key),
                    kind: KeyEventKind::Press | KeyEventKind::Repeat,
                    ..
                }) if state.tests_active => {
                    state.input_count = state.input_count.saturating_add(1);
                    let _ = state.tests.handle_key(key);
                }
                Event::Key(
                    key @ KeyEvent {
                        kind: KeyEventKind::Press | KeyEventKind::Repeat,
                        ..
                    },
                ) if state.logcat_active => {
                    state.input_count = state.input_count.saturating_add(1);
                    let action = state.logcat.handle_key(key);
                    if let Some(backend) = backend.as_deref_mut() {
                        let result = match action {
                            LogWorkspaceAction::None => Ok(()),
                            LogWorkspaceAction::ScopeChanged(device) => {
                                backend.set_device_scope(device)
                            }
                            LogWorkspaceAction::ProcessSelectionRequested => {
                                backend.select_process()
                            }
                            LogWorkspaceAction::CopyLine => backend.copy(false),
                            LogWorkspaceAction::CopyGroup => backend.copy(true),
                            LogWorkspaceAction::ExportRequested => backend.export(),
                            LogWorkspaceAction::RecordingToggled => backend.toggle_recording(),
                        };
                        if let Err(error) = result {
                            state.logcat.set_status(error);
                        }
                    }
                }
                Event::Key(KeyEvent {
                    kind: KeyEventKind::Press | KeyEventKind::Repeat,
                    ..
                }) => state.input_count = state.input_count.saturating_add(1),
                Event::Mouse(mouse) if state.logcat_active => state.logcat.handle_mouse(mouse),
                _ => {}
            }
        }
        if last_draw.elapsed() >= FRAME_INTERVAL
            && (!state.logcat_active || state.logcat.dirty)
            && (!state.tests_active || state.tests.dirty)
        {
            terminal.draw(|frame| render(frame, &mut state, theme))?;
            last_draw = Instant::now();
        }
    }
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

fn should_exit(event: &Event) -> bool {
    matches!(
        event,
        Event::Key(KeyEvent {
            code: KeyCode::Char('q') | KeyCode::Esc,
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
    frame.render_widget(
        Block::new().style(
            Style::default()
                .bg(theme.colors.background)
                .fg(theme.colors.text_primary),
        ),
        area,
    );
    let layout_mode = DashboardLayout::for_size(area.width, area.height);
    if layout_mode == DashboardLayout::ResizeWarning {
        render_resize_message(frame, area, theme);
        return;
    }

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(if layout_mode == DashboardLayout::SingleWorkspace {
                1
            } else {
                3
            }),
            Constraint::Length(if layout_mode == DashboardLayout::Full {
                3
            } else {
                1
            }),
            Constraint::Min(3),
            Constraint::Length(1),
        ])
        .split(area);

    let title = if theme.glyphs == GlyphMode::Ascii {
        "[DD] DexDeck"
    } else {
        "▰▱ DexDeck"
    };
    let project_status = if layout_mode == DashboardLayout::SingleWorkspace {
        "  project: detecting | model: unavailable"
    } else {
        "  project: detecting  model: unavailable"
    };
    let header = Paragraph::new(Line::from(vec![
        Span::styled(
            title,
            Style::default()
                .fg(theme.colors.action)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(project_status, Style::default().fg(theme.colors.text_muted)),
    ]));
    frame.render_widget(
        if layout_mode == DashboardLayout::SingleWorkspace {
            header
        } else {
            header.block(panel(" Control plane ", theme))
        },
        rows[0],
    );
    let actions = match layout_mode {
        DashboardLayout::Full => {
            "[Run] [Build] [Test] [Logs] [Devices] [Tasks]     Ctrl+P Commands"
        }
        DashboardLayout::Compact => "Run  Build  Test  Logs  Devices  Tasks  ^P Commands",
        DashboardLayout::SingleWorkspace => "^P Commands | ^T Tests | ^L Logs",
        DashboardLayout::ResizeWarning => unreachable!(),
    };
    frame.render_widget(
        Paragraph::new(actions).style(Style::default().fg(theme.colors.focus)),
        rows[1],
    );

    let workspace = if layout_mode == DashboardLayout::SingleWorkspace {
        rows[2]
    } else {
        let navigation_width = if layout_mode == DashboardLayout::Full {
            28
        } else {
            34
        };
        let columns = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(navigation_width),
                Constraint::Percentage(100 - navigation_width),
            ])
            .split(rows[2]);
        let navigation = if layout_mode == DashboardLayout::Full {
            "Modules\nVariants\nProfiles\n\nDevice: none\nSDK: detecting"
        } else {
            "Module / Variant\nDevice: none"
        };
        frame.render_widget(
            Paragraph::new(navigation)
                .style(Style::default().fg(theme.colors.text_primary))
                .block(panel(" Navigation ", theme)),
            columns[0],
        );
        columns[1]
    };
    if state.logcat_active {
        state.logcat.render(frame, workspace, theme);
    } else if state.tests_active {
        state.tests.render(frame, workspace, theme);
    } else {
        frame.render_widget(
            Paragraph::new("Project overview\n\nWaiting for project discovery.")
                .style(Style::default().fg(theme.colors.text_primary))
                .block(panel(" Workspace ", theme)),
            workspace,
        );
    }

    let size = state.last_size.map_or_else(
        || format!("{}x{}", area.width, area.height),
        |(w, h)| format!("{w}x{h}"),
    );
    frame.render_widget(
        Paragraph::new(format!(
            "OK Ready | {size} | {:?} | input: {} | q quit",
            layout_mode, state.input_count
        ))
        .style(Style::default().fg(theme.colors.text_muted)),
        rows[3],
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
            border::PLAIN
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
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;

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
        assert!(should_exit(&key(KeyCode::Esc, KeyModifiers::NONE)));
        assert!(should_exit(&key(KeyCode::Char('c'), KeyModifiers::CONTROL)));
        assert!(!should_exit(&key(KeyCode::Char('x'), KeyModifiers::NONE)));
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
