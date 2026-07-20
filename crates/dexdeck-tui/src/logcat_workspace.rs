use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseEvent, MouseEventKind};
use dexdeck_core::{ByteBoundedLogBuffer, CompiledLogFilter, LogFilterSpec, LogViewState};
use dexdeck_protocol::{LogRecord, LogTextSearch};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::LazuliTheme;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum LogOverlay {
    #[default]
    None,
    Search,
    Filters,
    Scope,
    Process,
    Export,
    Recording,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LogWorkspaceAction {
    None,
    ScopeChanged(bool),
    ProcessSelectionRequested,
    CopyLine,
    CopyGroup,
    ExportRequested,
    RecordingToggled,
}

#[derive(Debug)]
pub struct LogcatWorkspace {
    buffer: ByteBoundedLogBuffer,
    filter: CompiledLogFilter,
    filter_spec: LogFilterSpec,
    view: LogViewState,
    paused_at_sequence: Option<u64>,
    scroll_from_end: usize,
    pub overlay: LogOverlay,
    pub device_scope: bool,
    pub process: Option<String>,
    pub recording: bool,
    pub exporting: bool,
    pub status: String,
    pub dirty: bool,
}

impl Default for LogcatWorkspace {
    fn default() -> Self {
        Self {
            buffer: ByteBoundedLogBuffer::default(),
            filter: CompiledLogFilter::default(),
            filter_spec: LogFilterSpec::default(),
            view: LogViewState {
                follow: true,
                ..LogViewState::default()
            },
            paused_at_sequence: None,
            scroll_from_end: 0,
            overlay: LogOverlay::None,
            device_scope: false,
            process: None,
            recording: false,
            exporting: false,
            status: "Select a module, variant, package, and device to start Logcat.".into(),
            dirty: true,
        }
    }
}

impl LogcatWorkspace {
    pub fn new(buffer_bytes: usize) -> Result<Self, dexdeck_core::LogBufferError> {
        Ok(Self {
            buffer: ByteBoundedLogBuffer::new(buffer_bytes)?,
            ..Self::default()
        })
    }

    pub fn ingest(&mut self, records: Vec<LogRecord>) {
        for record in records {
            let _ = self.buffer.push(record);
        }
        self.dirty = true;
    }

    pub fn set_filter(&mut self, spec: LogFilterSpec) -> Result<(), String> {
        self.filter =
            CompiledLogFilter::compile(spec.clone()).map_err(|error| error.to_string())?;
        self.filter_spec = spec;
        self.scroll_from_end = 0;
        self.dirty = true;
        Ok(())
    }

    pub fn set_text_search(&mut self, query: &str) -> Result<(), String> {
        let mut spec = self.filter_spec.clone();
        spec.text_search = (!query.is_empty()).then(|| LogTextSearch::Plain(query.into()));
        self.set_filter(spec)
    }

    pub fn set_status(&mut self, status: impl Into<String>) {
        self.status = status.into();
        self.dirty = true;
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> LogWorkspaceAction {
        let action = match (key.code, key.modifiers) {
            (KeyCode::Char(' '), _) => {
                self.view.toggle_pause();
                self.paused_at_sequence = self
                    .view
                    .paused
                    .then(|| self.buffer.iter().next_back().map(|entry| entry.sequence))
                    .flatten();
                LogWorkspaceAction::None
            }
            (KeyCode::Char('c'), KeyModifiers::NONE) => {
                self.view.clear(&mut self.buffer);
                self.paused_at_sequence = None;
                self.scroll_from_end = 0;
                LogWorkspaceAction::None
            }
            (KeyCode::Char('/'), _) => {
                self.overlay = LogOverlay::Search;
                LogWorkspaceAction::None
            }
            (KeyCode::Char('f'), _) => {
                self.overlay = LogOverlay::Filters;
                LogWorkspaceAction::None
            }
            (KeyCode::Char('s'), _) => {
                self.device_scope = !self.device_scope;
                self.overlay = LogOverlay::Scope;
                LogWorkspaceAction::ScopeChanged(self.device_scope)
            }
            (KeyCode::Char('p'), _) => {
                self.overlay = LogOverlay::Process;
                LogWorkspaceAction::ProcessSelectionRequested
            }
            (KeyCode::Char('n'), KeyModifiers::SHIFT) => {
                self.navigate_error(true);
                LogWorkspaceAction::None
            }
            (KeyCode::Char('n'), _) => {
                self.navigate_error(false);
                LogWorkspaceAction::None
            }
            (KeyCode::Char('y'), KeyModifiers::SHIFT) => LogWorkspaceAction::CopyGroup,
            (KeyCode::Char('y'), _) => LogWorkspaceAction::CopyLine,
            (KeyCode::Char('e'), _) => {
                self.overlay = LogOverlay::Export;
                self.exporting = true;
                LogWorkspaceAction::ExportRequested
            }
            (KeyCode::Char('r'), _) => {
                self.overlay = LogOverlay::Recording;
                self.recording = !self.recording;
                LogWorkspaceAction::RecordingToggled
            }
            (KeyCode::End, _) => {
                self.view.follow = true;
                self.scroll_from_end = 0;
                LogWorkspaceAction::None
            }
            (KeyCode::Up, _) | (KeyCode::PageUp, _) => {
                self.scroll_from_end = self.scroll_from_end.saturating_add(1);
                self.view.follow = false;
                LogWorkspaceAction::None
            }
            (KeyCode::Down, _) | (KeyCode::PageDown, _) => {
                self.scroll_from_end = self.scroll_from_end.saturating_sub(1);
                LogWorkspaceAction::None
            }
            (KeyCode::Esc, _) => {
                self.overlay = LogOverlay::None;
                LogWorkspaceAction::None
            }
            _ => LogWorkspaceAction::None,
        };
        self.dirty = true;
        action
    }

    pub fn handle_mouse(&mut self, mouse: MouseEvent) {
        match mouse.kind {
            MouseEventKind::ScrollUp => {
                self.scroll_from_end = self.scroll_from_end.saturating_add(3);
                self.view.follow = false;
            }
            MouseEventKind::ScrollDown => {
                self.scroll_from_end = self.scroll_from_end.saturating_sub(3);
                if self.scroll_from_end == 0 {
                    self.view.follow = true;
                }
            }
            _ => return,
        }
        self.dirty = true;
    }

    fn navigate_error(&mut self, reverse: bool) {
        let records = self.buffer.snapshot();
        self.view.select_next_error(&records, reverse);
        if let Some(selected) = self.view.selected_sequence
            && let Some(position) = records.iter().position(|entry| entry.sequence == selected)
        {
            self.scroll_from_end = records.len().saturating_sub(position + 1);
        }
    }

    pub fn render(&mut self, frame: &mut Frame<'_>, area: Rect, theme: LazuliTheme) {
        let stats = self.buffer.stats();
        let separator = if theme.glyphs == crate::GlyphMode::Ascii {
            "|"
        } else {
            "·"
        };
        let title = format!(
            " Logcat [{}{}] {}{} ",
            if self.device_scope {
                "device"
            } else {
                "application"
            },
            self.process
                .as_deref()
                .map_or(String::new(), |process| format!("/{process}")),
            if self.view.paused {
                "PAUSED"
            } else if self.view.follow {
                "FOLLOW"
            } else {
                "SCROLL"
            },
            if self.recording { " REC" } else { "" },
        );
        let rows = ratatui::layout::Layout::vertical([
            ratatui::layout::Constraint::Length(2),
            ratatui::layout::Constraint::Min(0),
            ratatui::layout::Constraint::Length(1),
        ])
        .split(area);
        frame.render_widget(
            Paragraph::new(vec![
                Line::styled(title.trim().to_owned(), theme.accent()),
                Line::styled("/ Search   f Filters   s Scope   p Process", theme.muted()),
            ]),
            rows[0],
        );
        let height = usize::from(rows[1].height);
        let paused_at = self.paused_at_sequence.unwrap_or(u64::MAX);
        let visible = self
            .buffer
            .iter()
            .filter(|entry| entry.sequence <= paused_at && self.filter.matches(&entry.record))
            .collect::<Vec<_>>();
        let end = visible.len().saturating_sub(self.scroll_from_end);
        let start = end.saturating_sub(height);
        let lines = visible[start..end]
            .iter()
            .map(|entry| {
                let selected = self.view.selected_sequence == Some(entry.sequence);
                Line::from(vec![
                    Span::styled(
                        format!("{} {:>5} ", entry.record.timestamp, entry.record.process_id),
                        Style::default().fg(theme.colors.text_muted),
                    ),
                    Span::styled(
                        format!("{:?} {}: ", entry.record.priority, entry.record.tag),
                        if selected {
                            Style::default()
                                .fg(theme.colors.focus)
                                .add_modifier(Modifier::BOLD)
                        } else {
                            Style::default().fg(theme.colors.text_primary)
                        },
                    ),
                    Span::raw(&entry.record.message),
                ])
            })
            .collect::<Vec<_>>();
        frame.render_widget(Paragraph::new(lines), rows[1]);
        frame.render_widget(
            Paragraph::new(format!(
                "{}  {separator}  {} entries  {separator}  {} bytes  {separator}  {} dropped  {separator}  e Export  r Record",
                self.status,
                stats.buffered_entries,
                stats.buffered_bytes,
                stats.dropped_entries_since_clear,
            ))
            .style(theme.muted()),
            rows[2],
        );
        self.dirty = false;
    }
}

#[cfg(test)]
mod tests {
    use dexdeck_protocol::LogPriority;
    use ratatui::{Terminal, backend::TestBackend};

    use super::*;
    use crate::{ColorCapability, GlyphMode};

    fn record(index: u32) -> LogRecord {
        LogRecord {
            timestamp: "07-16 12:00:00.000".into(),
            process_id: index,
            thread_id: index,
            user_id: None,
            priority: LogPriority::Info,
            tag: "App".into(),
            message: format!("line {index}"),
            package: Some("com.example".into()),
            process: Some("com.example".into()),
            continuation: false,
            crash_boundary: false,
            group_id: Some(u64::from(index)),
            marker: None,
            truncated: false,
        }
    }

    #[test]
    fn test_backend_renders_only_viewport_rows() -> Result<(), Box<dyn std::error::Error>> {
        let mut workspace = LogcatWorkspace::default();
        workspace.ingest((0..100).map(record).collect());
        let mut terminal = Terminal::new(TestBackend::new(100, 10))?;
        let theme = LazuliTheme::new(ColorCapability::NoColor, GlyphMode::Ascii);
        terminal.draw(|frame| workspace.render(frame, frame.area(), theme))?;
        let rendered = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();
        assert!(rendered.contains("line 99"));
        assert!(!rendered.contains("line 0 "));
        Ok(())
    }

    #[test]
    fn key_bindings_pause_clear_scope_and_follow() {
        let mut workspace = LogcatWorkspace::default();
        workspace.ingest(vec![record(1)]);
        let key = |code, modifiers| KeyEvent::new(code, modifiers);
        workspace.handle_key(key(KeyCode::Char(' '), KeyModifiers::NONE));
        assert!(workspace.view.paused);
        assert_eq!(
            workspace.handle_key(key(KeyCode::Char('s'), KeyModifiers::NONE)),
            LogWorkspaceAction::ScopeChanged(true)
        );
        workspace.handle_key(key(KeyCode::Char('c'), KeyModifiers::NONE));
        assert_eq!(workspace.buffer.stats().buffered_entries, 0);
        workspace.handle_key(key(KeyCode::End, KeyModifiers::NONE));
        assert!(workspace.view.follow);
    }

    #[test]
    fn plain_text_search_filters_without_discarding_records() -> Result<(), String> {
        let mut workspace = LogcatWorkspace::default();
        workspace.ingest(vec![record(1), record(2)]);
        workspace.set_text_search("line 2")?;

        let matches = workspace
            .buffer
            .iter()
            .filter(|entry| workspace.filter.matches(&entry.record))
            .count();
        assert_eq!(matches, 1);
        assert_eq!(workspace.buffer.stats().buffered_entries, 2);
        Ok(())
    }
}
