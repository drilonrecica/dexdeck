use std::collections::VecDeque;

use dexdeck_protocol::{Diagnostic, JobRecord, JobState};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::Line,
    widgets::{Block, Borders, Paragraph},
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
}

impl RunWorkspace {
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
        let block = Block::default()
            .title(" Run & jobs ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.colors.border));
        let inner = block.inner(area);
        frame.render_widget(block, area);
        let rows = Layout::vertical([
            Constraint::Length(3),
            Constraint::Percentage(35),
            Constraint::Min(3),
            Constraint::Length(1),
        ])
        .split(inner);
        frame.render_widget(
            Paragraph::new(format!(
                "project: {} | model: {}\nmodule: {} | variant: {} | device: {} | app: {}",
                self.project.as_deref().unwrap_or("not detected"),
                if self.model_status.is_empty() {
                    "unavailable"
                } else {
                    &self.model_status
                },
                self.module.as_deref().unwrap_or("select"),
                self.variant.as_deref().unwrap_or("select"),
                self.device.as_deref().unwrap_or("select"),
                self.application_id.as_deref().unwrap_or("unresolved")
            )),
            rows[0],
        );
        let jobs = self
            .queue
            .iter()
            .enumerate()
            .map(|(index, job)| {
                let state = match job.state {
                    JobState::Queued => "WAIT",
                    JobState::Starting => "START",
                    JobState::Running => "RUN",
                    JobState::Cancelling => "STOP",
                    JobState::Succeeded => "OK",
                    JobState::Failed => "FAIL",
                    JobState::Cancelled => "CANCEL",
                };
                Line::styled(
                    format!("{state} {:?} {}", job.kind, job.command_summary.join(" ")),
                    if index == self.selected_job.selected {
                        Style::default()
                            .fg(theme.colors.focus)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(theme.colors.text_primary)
                    },
                )
            })
            .collect::<Vec<_>>();
        frame.render_widget(Paragraph::new(jobs), rows[1]);
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
            "No job output. Choose explicit build, install, launch, or run action.".into()
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
        frame.render_widget(Paragraph::new(details), rows[2]);
        frame.render_widget(
            Paragraph::new(
                "m/v/d select  u model  b build  i install  l launch  r run  c cancel  o open",
            )
            .style(Style::default().fg(theme.colors.text_muted)),
            rows[3],
        );
        self.dirty = false;
    }
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
