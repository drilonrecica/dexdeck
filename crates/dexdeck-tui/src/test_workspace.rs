use dexdeck_protocol::{Diagnostic, TestCaseResult, TestOutcome, TestRunResult};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::Style,
    text::Line,
    text::Span,
    widgets::{Block, Paragraph},
};

use crate::LazuliTheme;

const RAW_OUTPUT_LIMIT: usize = 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TestWorkspaceAction {
    None,
    RerunAll,
    RerunFailed,
    RerunSelected,
    OpenSource,
    CopyFailure,
}

#[derive(Clone, Debug, Default)]
pub struct TestWorkspace {
    pub runs: Vec<TestRunResult>,
    pub diagnostics: Vec<Diagnostic>,
    pub selected_case: usize,
    pub show_raw: bool,
    pub raw_output: String,
    pub status: String,
    pub dirty: bool,
    case_rows: Vec<Rect>,
}

impl TestWorkspace {
    pub fn handle_click(&mut self, x: u16, y: u16) -> bool {
        let Some(index) = self.case_rows.iter().position(|area| contains(*area, x, y)) else {
            return false;
        };
        self.selected_case = index;
        self.dirty = true;
        true
    }

    pub fn set_result(&mut self, result: TestRunResult) {
        self.runs.push(result);
        self.selected_case = 0;
        self.dirty = true;
    }

    pub fn set_diagnostics(&mut self, diagnostics: Vec<Diagnostic>) {
        self.diagnostics = diagnostics;
        self.dirty = true;
    }

    pub fn append_raw(&mut self, output: &str) {
        self.raw_output.push_str(output);
        if self.raw_output.len() > RAW_OUTPUT_LIMIT {
            let remove = self.raw_output.len() - RAW_OUTPUT_LIMIT;
            let boundary = self
                .raw_output
                .char_indices()
                .find_map(|(index, _)| (index >= remove).then_some(index))
                .unwrap_or(self.raw_output.len());
            self.raw_output.drain(..boundary);
        }
        self.dirty = true;
    }

    #[must_use]
    pub fn selected(&self) -> Option<&TestCaseResult> {
        self.runs.last()?.cases.get(self.selected_case)
    }

    pub fn handle_key(&mut self, key: char) -> TestWorkspaceAction {
        let action = match key {
            'j' => {
                let count = self.runs.last().map_or(0, |run| run.cases.len());
                self.selected_case = (self.selected_case + 1).min(count.saturating_sub(1));
                TestWorkspaceAction::None
            }
            'k' => {
                self.selected_case = self.selected_case.saturating_sub(1);
                TestWorkspaceAction::None
            }
            'a' => TestWorkspaceAction::RerunAll,
            'f' => TestWorkspaceAction::RerunFailed,
            'r' => TestWorkspaceAction::RerunSelected,
            'o' => TestWorkspaceAction::OpenSource,
            'y' => TestWorkspaceAction::CopyFailure,
            'x' => {
                self.show_raw = !self.show_raw;
                TestWorkspaceAction::None
            }
            _ => TestWorkspaceAction::None,
        };
        self.dirty = true;
        action
    }

    pub fn render(&mut self, frame: &mut Frame<'_>, area: Rect, theme: LazuliTheme) {
        let rows = Layout::vertical([
            Constraint::Length(2),
            Constraint::Percentage(45),
            Constraint::Min(3),
            Constraint::Length(1),
        ])
        .split(area);
        let summary = self.runs.last().map_or_else(
            || "No test result. Run a local or instrumentation target.".into(),
            |run| {
                format!(
                    "{} passed  {} failed  {} skipped  {} ms",
                    run.summary.passed,
                    run.summary.failed,
                    run.summary.skipped,
                    run.summary.duration_ms
                )
            },
        );
        frame.render_widget(
            Paragraph::new(vec![
                Line::styled("Test results", theme.accent()),
                Line::raw(summary),
            ]),
            rows[0],
        );
        let cases = self
            .runs
            .last()
            .map(|run| {
                run.cases
                    .iter()
                    .enumerate()
                    .map(|(index, case)| {
                        let selected = index == self.selected_case;
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
                                format!(
                                    "{marker} {} ",
                                    match case.outcome {
                                        TestOutcome::Passed => "Passed",
                                        TestOutcome::Failed => "Failed",
                                        TestOutcome::Skipped => "Skipped",
                                    }
                                ),
                                if selected {
                                    theme.selected()
                                } else {
                                    theme.muted()
                                },
                            ),
                            Span::styled(
                                format!("{}::{} ({} ms)", case.class, case.name, case.duration_ms),
                                if selected {
                                    theme.selected()
                                } else {
                                    Style::default().fg(theme.colors.text_primary)
                                },
                            ),
                        ])
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let cases_block = Block::new().title(Line::styled("Cases", theme.accent()));
        let cases_area = cases_block.inner(rows[1]);
        self.case_rows = (0..cases.len().min(usize::from(cases_area.height)))
            .map(|index| {
                Rect::new(
                    cases_area.x,
                    cases_area
                        .y
                        .saturating_add(u16::try_from(index).unwrap_or(u16::MAX)),
                    cases_area.width,
                    1,
                )
            })
            .collect();
        frame.render_widget(Paragraph::new(cases).block(cases_block), rows[1]);
        let detail = if self.show_raw {
            self.raw_output.clone()
        } else if let Some(case) = self.selected() {
            format!(
                "{}\n{}",
                case.failure_message.as_deref().unwrap_or("No failure"),
                case.stack_trace.as_deref().unwrap_or("")
            )
        } else if let Some(diagnostic) = self.diagnostics.first() {
            diagnostic.message.clone()
        } else {
            "No failure or diagnostic selected.".into()
        };
        frame.render_widget(
            Paragraph::new(detail)
                .block(Block::new().title(Line::styled("Details", theme.accent()))),
            rows[2],
        );
        frame.render_widget(
            Paragraph::new("Up/Down Select case  Ctrl+P Commands").style(theme.muted()),
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
    use dexdeck_protocol::{TestRunSummary, TestSelection};
    use ratatui::{Terminal, backend::TestBackend};

    use super::*;
    use crate::{ColorCapability, GlyphMode};

    #[test]
    fn renders_counts_and_failure_detail() -> Result<(), Box<dyn std::error::Error>> {
        let mut workspace = TestWorkspace::default();
        workspace.set_result(TestRunResult {
            selection: TestSelection::default(),
            summary: TestRunSummary {
                passed: 1,
                failed: 1,
                skipped: 0,
                duration_ms: 3,
            },
            cases: vec![TestCaseResult {
                suite: "suite".into(),
                class: "a.Example".into(),
                name: "fails".into(),
                outcome: TestOutcome::Failed,
                duration_ms: 3,
                failure_message: Some("boom".into()),
                stack_trace: Some("stack".into()),
                source: None,
            }],
        });
        let mut terminal = Terminal::new(TestBackend::new(100, 20))?;
        let theme = LazuliTheme::new(ColorCapability::NoColor, GlyphMode::Ascii);
        terminal.draw(|frame| workspace.render(frame, frame.area(), theme))?;
        let rendered = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();
        assert!(rendered.contains("boom"));
        Ok(())
    }
}
