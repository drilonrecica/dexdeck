use std::{
    io::{self, Stdout, Write},
    panic,
    sync::Arc,
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

use crate::{ColorCapability, GlyphMode, LazuliTheme};

pub const MINIMUM_WIDTH: u16 = 40;
pub const MINIMUM_HEIGHT: u16 = 10;

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

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ShellOptions {
    pub no_color: bool,
    pub ascii: bool,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct ShellState {
    input_count: u64,
    last_size: Option<(u16, u16)>,
}

pub fn run(options: ShellOptions) -> Result<(), ShellError> {
    let theme = LazuliTheme::new(
        ColorCapability::detect(options.no_color),
        if options.ascii {
            GlyphMode::Ascii
        } else {
            GlyphMode::Unicode
        },
    );
    let mut session = TerminalSession::enter()?;
    run_loop(session.terminal_mut(), theme)
}

fn run_loop<B: Backend>(terminal: &mut Terminal<B>, theme: LazuliTheme) -> Result<(), ShellError> {
    let mut state = ShellState::default();
    terminal.draw(|frame| render(frame, &state, theme))?;
    loop {
        let event = event::read()?;
        if should_exit(&event) {
            return Ok(());
        }
        match event {
            Event::Resize(width, height) => state.last_size = Some((width, height)),
            Event::Key(KeyEvent {
                kind: KeyEventKind::Press | KeyEventKind::Repeat,
                ..
            }) => state.input_count = state.input_count.saturating_add(1),
            _ => {}
        }
        terminal.draw(|frame| render(frame, &state, theme))?;
    }
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

fn render(frame: &mut Frame<'_>, state: &ShellState, theme: LazuliTheme) {
    let area = frame.area();
    frame.render_widget(
        Block::new().style(
            Style::default()
                .bg(theme.colors.background)
                .fg(theme.colors.text_primary),
        ),
        area,
    );
    if area.width < MINIMUM_WIDTH || area.height < MINIMUM_HEIGHT {
        render_resize_message(frame, area, theme);
        return;
    }

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Min(3),
            Constraint::Length(1),
        ])
        .split(area);

    let title = if theme.glyphs == GlyphMode::Ascii {
        "[DD] DexDeck"
    } else {
        "▰▱ DexDeck"
    };
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                title,
                Style::default()
                    .fg(theme.colors.action)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "  project: detecting  model: unavailable",
                Style::default().fg(theme.colors.text_muted),
            ),
        ]))
        .block(panel(" Control plane ", theme)),
        rows[0],
    );
    frame.render_widget(
        Paragraph::new("[Run] [Build] [Test] [Logs] [Devices] [Tasks]     Ctrl+P Commands")
            .style(Style::default().fg(theme.colors.focus))
            .block(panel(" Actions ", theme)),
        rows[1],
    );

    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(28), Constraint::Percentage(72)])
        .split(rows[2]);
    frame.render_widget(
        Paragraph::new("Modules\nVariants\nProfiles\n\nDevice: none")
            .style(Style::default().fg(theme.colors.text_primary))
            .block(panel(" Navigation ", theme)),
        columns[0],
    );
    frame.render_widget(
        Paragraph::new("Project overview\n\nWaiting for project discovery.")
            .style(Style::default().fg(theme.colors.text_primary))
            .block(panel(" Workspace ", theme)),
        columns[1],
    );

    let size = state.last_size.map_or_else(
        || format!("{}x{}", area.width, area.height),
        |(w, h)| format!("{w}x{h}"),
    );
    frame.render_widget(
        Paragraph::new(format!(
            "Ready | {size} | input events: {} | q quit",
            state.input_count
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
    use ratatui::{Terminal, backend::TestBackend, buffer::Buffer};

    use super::*;

    #[test]
    fn minimum_size_renders_only_resize_guidance() -> Result<(), Box<dyn std::error::Error>> {
        let backend = TestBackend::new(39, 3);
        let mut terminal = Terminal::new(backend)?;
        let theme = LazuliTheme::new(ColorCapability::NoColor, GlyphMode::Ascii);
        terminal.draw(|frame| render(frame, &ShellState::default(), theme))?;

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
}
