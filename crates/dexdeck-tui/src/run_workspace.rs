use std::collections::VecDeque;

use dexdeck_protocol::{Diagnostic, JobRecord, JobState};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::Style,
    text::Line,
    text::Span,
    widgets::Paragraph,
};

use crate::{LazuliTheme, VirtualList};

pub const RUN_OUTPUT_LIMIT: usize = 1024 * 1024;
pub const RUN_HISTORY_LIMIT: usize = 50;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RunWorkspaceAction {
    None,
    SelectModule,
    SelectVariant,
    SelectDevice,
    RefreshModel,
    Build,
    Install,
    Launch,
    Run,
    CancelSelected,
    OpenDiagnostic,
}

#[derive(Clone, Debug, Default)]
pub struct RunWorkspace {
    pub project: Option<String>,
    pub module: Option<String>,
    pub variant: Option<String>,
    pub device: Option<String>,
    pub application_id: Option<String>,
    pub model_status: String,
    pub queue: Vec<JobRecord>,
    pub history: VecDeque<JobRecord>,
    pub diagnostics: Vec<Diagnostic>,
    pub output: VecDeque<String>,
    pub output_bytes: usize,
    pub selected_job: VirtualList,
    pub dirty: bool,
    job_rows: Vec<Rect>,
}

impl RunWorkspace {
    pub fn handle_click(&mut self, x: u16, y: u16) -> bool {
        let Some(index) = self.job_rows.iter().position(|area| contains(*area, x, y)) else {
            return false;
        };
        self.selected_job.selected = index.min(self.queue.len().saturating_sub(1));
        self.dirty = true;
        true
    }

    pub fn set_selection(
        &mut self,
        project: Option<String>,
        module: Option<String>,
        variant: Option<String>,
        device: Option<String>,
        application_id: Option<String>,
    ) {
        self.project = project;
        self.module = module;
        self.variant = variant;
        self.device = device;
        self.application_id = application_id;
        self.dirty = true;
    }

    pub fn set_queue(&mut self, queue: Vec<JobRecord>) {
        self.queue = queue;
        self.selected_job.select(
            self.selected_job.selected,
            self.queue.len(),
            self.queue.len().max(1),
        );
        self.dirty = true;
    }

    pub fn finish_job(&mut self, job: JobRecord) {
        self.queue.retain(|queued| queued.id != job.id);
        self.diagnostics.extend(job.diagnostics.iter().cloned());
        self.history.push_front(job);
        self.history.truncate(RUN_HISTORY_LIMIT);
        self.dirty = true;
    }

    pub fn append_output(&mut self, text: &str) {
        for line in text.lines() {
            self.output_bytes = self.output_bytes.saturating_add(line.len());
            self.output.push_back(line.to_owned());
        }
        while self.output_bytes > RUN_OUTPUT_LIMIT {
            let Some(line) = self.output.pop_front() else {
                break;
            };
            self.output_bytes = self.output_bytes.saturating_sub(line.len());
        }
        self.dirty = true;
    }

    pub fn handle_key(&mut self, key: char) -> RunWorkspaceAction {
        let action = match key {
            'm' => RunWorkspaceAction::SelectModule,
            'v' => RunWorkspaceAction::SelectVariant,
            'd' => RunWorkspaceAction::SelectDevice,
            'u' => RunWorkspaceAction::RefreshModel,
            'b' => RunWorkspaceAction::Build,
            'i' => RunWorkspaceAction::Install,
            'l' => RunWorkspaceAction::Launch,
            'r' => RunWorkspaceAction::Run,
            'c' => RunWorkspaceAction::CancelSelected,
            'o' => RunWorkspaceAction::OpenDiagnostic,
            'j' => {
                self.selected_job.select(
                    self.selected_job.selected.saturating_add(1),
                    self.queue.len(),
                    10,
                );
                RunWorkspaceAction::None
            }
            'k' => {
                self.selected_job.select(
                    self.selected_job.selected.saturating_sub(1),
                    self.queue.len(),
                    10,
                );
                RunWorkspaceAction::None
            }
            _ => RunWorkspaceAction::None,
        };
        self.dirty = true;
        action
    }

    pub fn render(&mut self, frame: &mut Frame<'_>, area: Rect, theme: LazuliTheme) {
        let rows = Layout::vertical([
            Constraint::Length(4),
            Constraint::Percentage(35),
            Constraint::Min(3),
            Constraint::Length(1),
        ])
        .split(area);
        let separator = if theme.glyphs == crate::GlyphMode::Ascii {
            "|"
        } else {
            "·"
        };
        frame.render_widget(
            Paragraph::new(vec![
                Line::styled("Target", theme.accent()),
                Line::from(format!(
                    "{}  {separator}  {} / {}  {separator}  {}",
                    self.project.as_deref().unwrap_or("Project not detected"),
                    self.module.as_deref().unwrap_or("Select module"),
                    self.variant.as_deref().unwrap_or("Select variant"),
                    self.device.as_deref().unwrap_or("Select device"),
                )),
                Line::styled(
                    format!(
                        "Model: {}  {separator}  App: {}",
                        if self.model_status.is_empty() {
                            "Unavailable"
                        } else {
                            &self.model_status
                        },
                        self.application_id.as_deref().unwrap_or("Unresolved")
                    ),
                    theme.muted(),
                ),
            ]),
            rows[0],
        );
        let jobs = self
            .queue
            .iter()
            .enumerate()
            .map(|(index, job)| {
                let state = match job.state {
                    JobState::Queued => "Queued",
                    JobState::Starting => "Starting",
                    JobState::Running => "Running",
                    JobState::Cancelling => "Cancelling",
                    JobState::Succeeded => "Succeeded",
                    JobState::Failed => "Failed",
                    JobState::Cancelled => "Cancelled",
                };
                let selected = index == self.selected_job.selected;
                let marker = if selected {
                    if theme.glyphs == crate::GlyphMode::Ascii {
                        ">"
                    } else {
                        "▌"
                    }
                } else {
                    " "
                };
                Line::from(vec![
                    Span::styled(
                        format!("{marker} {state:<10}"),
                        if selected {
                            theme.selected()
                        } else {
                            theme.muted()
                        },
                    ),
                    Span::styled(
                        format!(" {:?}  {}", job.kind, job.command_summary.join(" ")),
                        if selected {
                            theme.selected()
                        } else {
                            Style::default().fg(theme.colors.text_primary)
                        },
                    ),
                ])
            })
            .collect::<Vec<_>>();
        let jobs_block = ratatui::widgets::Block::new().title(Line::styled("Jobs", theme.accent()));
        let jobs_area = jobs_block.inner(rows[1]);
        self.job_rows = (0..jobs.len().min(usize::from(jobs_area.height)))
            .map(|index| {
                Rect::new(
                    jobs_area.x,
                    jobs_area
                        .y
                        .saturating_add(u16::try_from(index).unwrap_or(u16::MAX)),
                    jobs_area.width,
                    1,
                )
            })
            .collect();
        let jobs = if jobs.is_empty() {
            vec![Line::styled("No active jobs", theme.muted())]
        } else {
            jobs
        };
        frame.render_widget(Paragraph::new(jobs).block(jobs_block), rows[1]);
        let details = if let Some(diagnostic) = self.diagnostics.last() {
            format!(
                "{:?}: {}\n{}",
                diagnostic.severity,
                diagnostic.message,
                self.output
                    .iter()
                    .rev()
                    .take(8)
                    .rev()
                    .cloned()
                    .collect::<Vec<_>>()
                    .join("\n")
            )
        } else if self.output.is_empty() {
            "No job output. Choose Build, Install, Launch, or Run.".into()
        } else {
            self.output
                .iter()
                .rev()
                .take(12)
                .rev()
                .cloned()
                .collect::<Vec<_>>()
                .join("\n")
        };
        frame.render_widget(
            Paragraph::new(details).block(
                ratatui::widgets::Block::new().title(Line::styled("Output", theme.accent())),
            ),
            rows[2],
        );
        frame.render_widget(
            Paragraph::new("Up/Down Select job  Ctrl+P Commands").style(theme.muted()),
            rows[3],
        );
        self.dirty = false;
    }
}

fn contains(area: Rect, x: u16, y: u16) -> bool {
    x >= area.x
        && x < area.x.saturating_add(area.width)
        && y >= area.y
        && y < area.y.saturating_add(area.height)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_is_byte_bounded() {
        let mut workspace = RunWorkspace::default();
        workspace.append_output(&"x".repeat(RUN_OUTPUT_LIMIT + 10));
        assert!(workspace.output_bytes <= RUN_OUTPUT_LIMIT);
    }
}
