use std::io::{self, IsTerminal, Write};
use std::time::Duration;

use anyhow::{bail, Context, Result};
use crossterm::{
    cursor::{Hide, MoveTo, Show},
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen},
};

use agentboard_core::{model::Workspace, CancellationToken};

use crate::store::{source_snapshots, SnapshotState, SourceSnapshot};

pub fn require_dashboard_terminals(stdin: bool, stdout: bool) -> Result<()> {
    if stdin && stdout {
        Ok(())
    } else {
        bail!("Dashboard requires interactive stdin and stdout")
    }
}

pub fn dashboard(ws: &Workspace, cancellation: CancellationToken) -> Result<()> {
    require_dashboard_terminals(io::stdin().is_terminal(), io::stdout().is_terminal())?;
    let mut terminal = TerminalSession::enter()?;
    let mut selected = 0_usize;
    let mut last_view = None;

    loop {
        if cancellation.is_cancelled() {
            return Err(crate::runtime::InvocationCancelled.into());
        }
        let width = terminal::size().context("read Dashboard terminal size")?.0 as usize;
        let snapshots = source_snapshots(ws)?;
        if selected >= snapshots.len() {
            selected = 0;
        }
        let view = render_view(&snapshots, selected, width);
        if visible_view_changed(last_view.as_deref(), &view) {
            execute!(terminal.writer, MoveTo(0, 0), Clear(ClearType::All))?;
            terminal.writer.write_all(view.as_bytes())?;
            terminal.writer.flush()?;
            last_view = Some(view);
        }

        if event::poll(Duration::from_secs(1)).context("poll Dashboard input")? {
            while event::poll(Duration::ZERO).context("poll Dashboard input")? {
                let Event::Key(key) = event::read().context("read Dashboard input")? else {
                    continue;
                };
                if key.kind != KeyEventKind::Press {
                    continue;
                }
                match dashboard_command(key.code, key.modifiers) {
                    DashboardCommand::Quit => return Ok(()),
                    DashboardCommand::Previous if !snapshots.is_empty() => {
                        selected = previous_source(selected, snapshots.len());
                        last_view = None;
                    }
                    DashboardCommand::Next if !snapshots.is_empty() => {
                        selected = next_source(selected, snapshots.len());
                        last_view = None;
                    }
                    DashboardCommand::Previous
                    | DashboardCommand::Next
                    | DashboardCommand::Ignore => {}
                }
            }
        }
    }
}

struct TerminalSession {
    writer: io::Stdout,
    active: bool,
}

impl TerminalSession {
    fn enter() -> Result<Self> {
        terminal::enable_raw_mode().context("enable Dashboard terminal mode")?;
        let mut session = Self {
            writer: io::stdout(),
            active: true,
        };
        if let Err(error) = execute!(session.writer, EnterAlternateScreen, Hide) {
            session.restore();
            return Err(error).context("enter Dashboard terminal screen");
        }
        Ok(session)
    }

    fn restore(&mut self) {
        if !self.active {
            return;
        }
        let _ = terminal::disable_raw_mode();
        let _ = execute!(self.writer, Show, LeaveAlternateScreen);
        let _ = self.writer.flush();
        self.active = false;
    }
}

impl Drop for TerminalSession {
    fn drop(&mut self) {
        self.restore();
    }
}

fn render_view(snapshots: &[SourceSnapshot], selected: usize, width: usize) -> String {
    if snapshots.is_empty() {
        return "agentboard dashboard\n\nWorkspace has no configured Sources.\n\nq quit\n".into();
    }

    let snapshot = &snapshots[selected];
    let mut output = format!(
        "agentboard dashboard  Source {}/{}: {}\nControls: Left/Right or h/l change Source, q quit\n\n",
        selected + 1,
        snapshots.len(),
        snapshot.source_id
    );
    match snapshot.state {
        SnapshotState::Missing => {
            output
                .push_str("This Source has no current Snapshot. Run the Workspace successfully.\n");
        }
        SnapshotState::Ready if snapshot.items.is_empty() => {
            output.push_str("This Source has a valid current Snapshot with zero Items.\n");
        }
        SnapshotState::Ready => render_table(&mut output, snapshot, width),
    }
    output
}

fn render_table(output: &mut String, snapshot: &SourceSnapshot, width: usize) {
    let reference_width = snapshot
        .items
        .iter()
        .map(|item| display_width(&item.item.reference_id))
        .max()
        .unwrap_or(0)
        .max(display_width("Reference ID"));
    let status_width = snapshot
        .items
        .iter()
        .map(|item| display_width(&item.item.status))
        .max()
        .unwrap_or(0)
        .max(display_width("Status"));
    let result_width = snapshot
        .items
        .iter()
        .map(|item| display_width(item.result))
        .max()
        .unwrap_or(0)
        .max(display_width("Action Plan Result"));
    let fixed_width = reference_width + status_width + result_width + 6;
    let title_width = width.saturating_sub(fixed_width);
    if title_width < 4 {
        output.push_str(&format!(
            "Terminal is too narrow. Minimum width: {} columns.\n",
            fixed_width + 4
        ));
        return;
    }

    output.push_str(&format!(
        "{}  {}  {}  {}\n",
        cell("Reference ID", reference_width),
        cell("Title", title_width),
        cell("Status", status_width),
        cell("Action Plan Result", result_width),
    ));
    for item in &snapshot.items {
        output.push_str(&format!(
            "{}  {}  {}  {}\n",
            cell(&item.item.reference_id, reference_width),
            cell(&truncate_title(&item.item.title, title_width), title_width),
            cell(&item.item.status, status_width),
            cell(item.result, result_width),
        ));
    }
}

fn truncate_title(title: &str, width: usize) -> String {
    let title = title.replace(['\n', '\r'], " ");
    if display_width(&title) <= width {
        return title;
    }
    title
        .chars()
        .take(width.saturating_sub(1))
        .chain(['…'])
        .collect()
}

fn cell(value: &str, width: usize) -> String {
    let padding = width.saturating_sub(display_width(value));
    format!("{value}{}", " ".repeat(padding))
}

fn display_width(value: &str) -> usize {
    value.chars().count()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DashboardCommand {
    Quit,
    Previous,
    Next,
    Ignore,
}

fn dashboard_command(code: KeyCode, modifiers: KeyModifiers) -> DashboardCommand {
    match code {
        KeyCode::Char('q') => DashboardCommand::Quit,
        KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => DashboardCommand::Quit,
        KeyCode::Left | KeyCode::Char('h') => DashboardCommand::Previous,
        KeyCode::Right | KeyCode::Char('l') => DashboardCommand::Next,
        _ => DashboardCommand::Ignore,
    }
}

fn previous_source(selected: usize, source_count: usize) -> usize {
    selected.checked_sub(1).unwrap_or(source_count - 1)
}

fn next_source(selected: usize, source_count: usize) -> usize {
    (selected + 1) % source_count
}

fn visible_view_changed(previous: Option<&str>, current: &str) -> bool {
    previous != Some(current)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::SnapshotItem;
    use agentboard_core::model::Item;
    use serde_json::json;

    fn snapshot(state: SnapshotState, items: Vec<SnapshotItem>) -> SourceSnapshot {
        SourceSnapshot {
            source_id: "issues".into(),
            state,
            items,
        }
    }

    fn item(reference_id: &str, title: &str) -> SnapshotItem {
        SnapshotItem {
            item: Item {
                id: reference_id.into(),
                reference_id: reference_id.into(),
                title: title.into(),
                status: "ready".into(),
                url: "https://example.test".into(),
                source_id: "issues".into(),
                source_kind: "test".into(),
                raw: json!({}),
            },
            result: "pending",
        }
    }

    #[test]
    fn rejects_non_interactive_streams() {
        assert!(require_dashboard_terminals(false, true).is_err());
        assert!(require_dashboard_terminals(true, false).is_err());
        assert!(require_dashboard_terminals(true, true).is_ok());
    }

    #[test]
    fn renders_distinct_empty_states() {
        assert!(render_view(&[], 0, 80).contains("no configured Sources"));
        assert!(
            render_view(&[snapshot(SnapshotState::Missing, vec![])], 0, 80)
                .contains("no current Snapshot")
        );
        assert!(
            render_view(&[snapshot(SnapshotState::Ready, vec![])], 0, 80).contains("zero Items")
        );
    }

    #[test]
    fn wraps_source_navigation() {
        let snapshots = vec![
            snapshot(SnapshotState::Ready, vec![]),
            SourceSnapshot {
                source_id: "mine".into(),
                state: SnapshotState::Ready,
                items: vec![],
            },
        ];
        assert!(render_view(&snapshots, 0, 80).contains("issues"));
        assert!(render_view(&snapshots, 1, 80).contains("mine"));
    }

    #[test]
    fn truncates_only_the_title_when_width_is_limited() {
        let text = render_view(
            &[snapshot(
                SnapshotState::Ready,
                vec![item("AB-123", "A very long title that must be truncated")],
            )],
            0,
            52,
        );
        assert!(text.contains("AB-123"));
        assert!(text.contains("ready"));
        assert!(text.contains("pending"));
        assert!(text.contains('…'));
    }

    #[test]
    fn navigation_commands_wrap_and_quit() {
        assert_eq!(previous_source(0, 3), 2);
        assert_eq!(next_source(2, 3), 0);
        assert_eq!(
            dashboard_command(KeyCode::Char('c'), KeyModifiers::CONTROL),
            DashboardCommand::Quit
        );
        assert_eq!(
            dashboard_command(KeyCode::Left, KeyModifiers::NONE),
            DashboardCommand::Previous
        );
    }

    #[test]
    fn unchanged_visible_view_does_not_redraw() {
        assert!(!visible_view_changed(Some("same"), "same"));
        assert!(visible_view_changed(Some("old"), "new"));
        assert!(visible_view_changed(None, "first"));
    }

    #[test]
    fn table_fits_width_or_reports_minimum_width() {
        let snapshots = [snapshot(
            SnapshotState::Ready,
            vec![item("AB-123", "A long title")],
        )];
        let narrow = render_view(&snapshots, 0, 10);
        assert!(narrow.contains("Minimum width"));

        let width = 80;
        let rendered = render_view(&snapshots, 0, width);
        assert!(rendered.lines().all(|line| display_width(line) <= width));
    }
}
