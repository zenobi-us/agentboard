use std::io::{self, IsTerminal};
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};
use chrono::Utc;
use crossterm::{
    event::{self, Event, KeyEventKind},
    terminal as crossterm_terminal,
};
use ratatui::layout::Rect;

use agentboard_core::{model::Workspace, CancellationToken};

use crate::store::{recent_events, source_snapshots, EventLogEntry};

mod input;
mod logo;
mod terminal;
mod view;

use input::{
    clicked_source, clicked_watch, dashboard_command, next_source, previous_source,
    DashboardCommand,
};
use terminal::TerminalSession;
use view::{
    dashboard_areas, dashboard_columns, render_view, view_signature, watch_button_area, Footer,
};

const WATCH_INTERVAL: Duration = Duration::from_secs(60);
const UI_TICK: Duration = Duration::from_millis(100);

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
    let mut watching = true;
    let mut loaded = false;
    let mut snapshots = Vec::new();
    let mut event_records = Vec::new();
    let mut events_open = false;
    let mut last_view = None;
    let mut next_fetch = Instant::now();
    let mut poll_number = 0_u64;
    let mut last_poll_label = None;

    loop {
        if cancellation.is_cancelled() {
            return Err(crate::runtime::InvocationCancelled.into());
        }
        let (width, height) = crossterm_terminal::size().context("read Dashboard terminal size")?;
        if !loaded || (watching && Instant::now() >= next_fetch) {
            snapshots = source_snapshots(ws)?;
            event_records = recent_events(ws, 512)?;
            loaded = true;
            poll_number += 1;
            last_poll_label = Some(Utc::now().format("%H:%M:%S").to_string());
            next_fetch = Instant::now() + WATCH_INTERVAL;
        }
        if selected >= snapshots.len() {
            selected = 0;
        }

        let countdown = watching.then(|| next_fetch.saturating_duration_since(Instant::now()));
        let footer = watch_footer(poll_number, last_poll_label.as_deref(), countdown);
        let events = selected_source_events(&snapshots, selected, &event_records);
        let view = view_signature(
            &snapshots,
            selected,
            width as usize,
            watching,
            &footer,
            events_open,
            &events,
        );
        if visible_view_changed(last_view.as_deref(), &view) {
            terminal.terminal.draw(|frame| {
                render_view(
                    frame,
                    ws,
                    &snapshots,
                    selected,
                    watching,
                    Footer {
                        text: &footer,
                        events_open,
                        events: &events,
                    },
                )
            })?;
            last_view = Some(view);
        }

        let wait = if watching {
            next_fetch
                .saturating_duration_since(Instant::now())
                .min(UI_TICK)
        } else {
            UI_TICK
        };
        if event::poll(wait).context("poll Dashboard input")? {
            while event::poll(Duration::ZERO).context("poll Dashboard input")? {
                match event::read().context("read Dashboard input")? {
                    Event::Key(key) => {
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
                            DashboardCommand::ToggleEvents => {
                                events_open = !events_open;
                                last_view = None;
                            }
                            DashboardCommand::Previous
                            | DashboardCommand::Next
                            | DashboardCommand::Ignore => {}
                        }
                    }
                    Event::Mouse(mouse) => {
                        let areas = dashboard_areas(Rect::new(0, 0, width, height), events_open);
                        if clicked_watch(&mouse, watch_button_area(areas[0])) {
                            watching = !watching;
                            if watching {
                                next_fetch = Instant::now();
                            }
                            last_view = None;
                        } else if let Some(source) =
                            clicked_source(&mouse, dashboard_columns(areas[1])[0], ws)
                        {
                            selected = source;
                            last_view = None;
                        }
                    }
                    _ => {}
                }
            }
        }
    }
}

fn selected_source_events(
    snapshots: &[crate::store::SourceSnapshot],
    selected: usize,
    records: &[EventLogEntry],
) -> Vec<EventLogEntry> {
    let Some(source_id) = snapshots
        .get(selected)
        .map(|snapshot| snapshot.source_id.as_str())
    else {
        return Vec::new();
    };
    let mut events = records
        .iter()
        .filter(|event| event.source.as_deref() == Some(source_id))
        .rev()
        .take(6)
        .cloned()
        .collect::<Vec<_>>();
    events.reverse();
    events
}

fn watch_footer(
    poll_number: u64,
    last_poll_label: Option<&str>,
    countdown: Option<Duration>,
) -> String {
    let controls = "←/h Prev   →/l Next   e Events   q/Ctrl-C Quit";
    let last_poll = format!(
        "Last poll #{poll_number} at {}",
        last_poll_label.unwrap_or("--:--:--")
    );
    if let Some(remaining) = countdown {
        let filled = (remaining.as_millis() * 10)
            .div_ceil(WATCH_INTERVAL.as_millis())
            .min(10) as usize;
        return format!(
            "{controls}   ·   {last_poll}   ·   Next fetch in {:.1}s {}{}",
            remaining.as_secs_f64(),
            "█".repeat(filled),
            "░".repeat(10 - filled),
        );
    }
    format!("{controls}   ·   Watch paused   ·   {last_poll}")
}

fn visible_view_changed(previous: Option<&str>, current: &str) -> bool {
    previous != Some(current)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::{EventLogEntry, SnapshotItem, SnapshotState, SourceSnapshot};
    use agentboard_core::model::Item;
    use crossterm::event::{KeyCode, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
    use ratatui::{backend::TestBackend, buffer::Buffer, Terminal};
    use serde_json::json;

    fn snapshot(state: SnapshotState, items: Vec<SnapshotItem>) -> SourceSnapshot {
        SourceSnapshot {
            source_id: "issues".into(),
            state,
            collection: None,
            items,
            actions: vec![],
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

    fn rendered_view(snapshots: &[SourceSnapshot], selected: usize, width: usize) -> String {
        rendered_view_with_watch(snapshots, selected, width, true)
    }

    fn rendered_view_with_watch(
        snapshots: &[SourceSnapshot],
        selected: usize,
        width: usize,
        watching: bool,
    ) -> String {
        let workspace = workspace_for(snapshots);
        let backend = TestBackend::new(width as u16, 12);
        let mut terminal = Terminal::new(backend).unwrap();
        let frame = terminal
            .draw(|frame| {
                render_view(
                    frame,
                    &workspace,
                    snapshots,
                    selected,
                    watching,
                    Footer {
                        text: "test footer",
                        events_open: false,
                        events: &[],
                    },
                )
            })
            .unwrap();
        buffer_text(frame.buffer)
    }

    fn workspace_for(snapshots: &[SourceSnapshot]) -> Workspace {
        let sources = snapshots
            .iter()
            .map(|snapshot| {
                format!(
                    "[[sources]]\nid = {:?}\n[sources.source]\nkind = \"qmd\"\ncollections = [\"test\"]\nquery = \"ready\"\n",
                    snapshot.source_id
                )
            })
            .collect::<String>();
        let sources = if sources.is_empty() {
            "sources = []\n".to_owned()
        } else {
            sources
        };
        let registry = crate::cli::register_builtins().unwrap();
        let parsed = crate::config::parse_workspace(&sources, &registry).unwrap();
        Workspace {
            id: "test".into(),
            path: "test.toml".into(),
            sources: parsed.sources,
        }
    }

    fn workspace_with_actions() -> Workspace {
        let registry = crate::cli::register_builtins().unwrap();
        let parsed = crate::config::parse_workspace(
            r#"
[[sources]]
id = "issues"
[sources.source]
kind = "qmd"
collections = ["test"]
query = "ready"
[[sources.actions]]
id = "run"
uses = "agentboard/run-cmd"
[sources.actions.with]
cmd = "echo run"
[[sources.actions]]
id = "worktree"
uses = "agentboard/run-cmd"
[sources.actions.with]
cmd = "echo worktree"
"#,
            &registry,
        )
        .unwrap();
        Workspace {
            id: "test".into(),
            path: "test.toml".into(),
            sources: parsed.sources,
        }
    }

    fn buffer_text(buffer: &Buffer) -> String {
        let area = buffer.area();
        (0..area.height)
            .map(|y| {
                (0..area.width)
                    .map(|x| buffer[(x, y)].symbol())
                    .collect::<String>()
                    .trim_end()
                    .to_owned()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn rejects_non_interactive_streams() {
        assert!(require_dashboard_terminals(false, true).is_err());
        assert!(require_dashboard_terminals(true, false).is_err());
        assert!(require_dashboard_terminals(true, true).is_ok());
    }

    #[test]
    fn renders_distinct_empty_states() {
        assert!(rendered_view(&[], 0, 80).contains("no configured Sources"));
        assert!(
            rendered_view(&[snapshot(SnapshotState::Missing, vec![])], 0, 80)
                .contains("No current Snapshot")
        );
        assert!(
            rendered_view(&[snapshot(SnapshotState::Ready, vec![])], 0, 80)
                .contains("No issue tickets found")
        );
    }

    #[test]
    fn renders_workspace_tree_and_ticket_list() {
        let snapshots = [SourceSnapshot {
            source_id: "issues".into(),
            state: SnapshotState::Ready,
            collection: Some(crate::store::SourceCollectionStatus {
                state: crate::store::CollectionState::Failed,
                updated_at: "2026-01-01T00:00:00Z".into(),
                error: Some("query failed".into()),
            }),
            items: vec![item("AB-123", "Fix login timeout")],
            actions: vec![
                crate::store::ActionState::Success,
                crate::store::ActionState::Pending,
            ],
        }];
        let workspace = workspace_with_actions();
        let backend = TestBackend::new(100, 12);
        let mut terminal = Terminal::new(backend).unwrap();
        let frame = terminal
            .draw(|frame| {
                render_view(
                    frame,
                    &workspace,
                    &snapshots,
                    0,
                    true,
                    Footer {
                        text: "test footer",
                        events_open: false,
                        events: &[],
                    },
                )
            })
            .unwrap();
        let text = buffer_text(frame.buffer);

        assert!(text.contains("query: ready"));
        assert!(text.contains("run"));
        assert!(text.contains("worktree"));
        assert!(text.contains("AB-123"));
        assert!(text.contains("Fix login timeout"));
        assert!(text.contains("failed"));
        assert!(text.contains("⟳"));
    }

    #[test]
    fn renders_current_source_events_when_expanded() {
        let snapshots = [snapshot(SnapshotState::Ready, vec![])];
        let workspace = workspace_for(&snapshots);
        let events = [EventLogEntry {
            timestamp: "2026-01-01T12:34:56Z".into(),
            level: "info".into(),
            stage: "source.complete".into(),
            source: Some("issues".into()),
            kind: Some("github".into()),
            items: Some(12),
            error: None,
        }];
        let backend = TestBackend::new(100, 20);
        let mut terminal = Terminal::new(backend).unwrap();
        let frame = terminal
            .draw(|frame| {
                render_view(
                    frame,
                    &workspace,
                    &snapshots,
                    0,
                    true,
                    Footer {
                        text: "test footer",
                        events_open: true,
                        events: &events,
                    },
                )
            })
            .unwrap();
        let text = buffer_text(frame.buffer);

        assert!(text.contains("current Source"));
        assert!(text.contains("source.complete"));
        assert!(text.contains("source=issues kind=github items=12"));
    }

    #[test]
    fn renders_watch_button_state() {
        assert!(rendered_view_with_watch(&[], 0, 80, true).contains("[ Watch: ON ]"));
        assert!(rendered_view_with_watch(&[], 0, 80, false).contains("[ Watch: OFF ]"));
    }

    #[test]
    fn wraps_source_navigation() {
        let snapshots = vec![
            snapshot(SnapshotState::Ready, vec![]),
            SourceSnapshot {
                source_id: "mine".into(),
                state: SnapshotState::Ready,
                collection: None,
                items: vec![],
                actions: vec![],
            },
        ];
        assert!(rendered_view(&snapshots, 0, 80).contains("issues"));
        assert!(rendered_view(&snapshots, 1, 80).contains("mine"));
    }

    #[test]
    fn truncates_only_the_title_when_width_is_limited() {
        let text = rendered_view(
            &[snapshot(
                SnapshotState::Ready,
                vec![item("AB-123", "A very long title that must be truncated")],
            )],
            0,
            80,
        );
        assert!(text.contains("AB-123"));
        assert!(text.contains("ready"));
        assert!(text.contains("pending"));
        assert!(text.contains('…'));
    }

    #[test]
    fn mouse_clicks_select_source_tree_rows() {
        let snapshots = vec![
            snapshot(SnapshotState::Ready, vec![]),
            SourceSnapshot {
                source_id: "mine".into(),
                state: SnapshotState::Ready,
                collection: None,
                items: vec![],
                actions: vec![],
            },
        ];
        let workspace = workspace_for(&snapshots);
        let tree_area = dashboard_columns(dashboard_areas(Rect::new(0, 0, 80, 12), false)[1])[0];
        let first = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: tree_area.x + 1,
            row: tree_area.y + 1,
            modifiers: KeyModifiers::NONE,
        };
        let second = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: tree_area.x + 1,
            row: tree_area.y + 3,
            modifiers: KeyModifiers::NONE,
        };
        let outside = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Right),
            column: tree_area.x + 1,
            row: tree_area.y + 1,
            modifiers: KeyModifiers::NONE,
        };

        assert_eq!(clicked_source(&first, tree_area, &workspace), Some(0));
        assert_eq!(clicked_source(&second, tree_area, &workspace), Some(1));
        assert_eq!(clicked_source(&outside, tree_area, &workspace), None);
    }

    #[test]
    fn mouse_clicks_toggle_watch_button() {
        let button = watch_button_area(dashboard_areas(Rect::new(0, 0, 80, 12), false)[0]);
        let click = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: button.x + 1,
            row: button.y,
            modifiers: KeyModifiers::NONE,
        };
        let outside = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: button.x.saturating_sub(1),
            row: button.y,
            modifiers: KeyModifiers::NONE,
        };

        assert!(clicked_watch(&click, button));
        assert!(!clicked_watch(&outside, button));
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
        assert_eq!(
            dashboard_command(KeyCode::Char('e'), KeyModifiers::NONE),
            DashboardCommand::ToggleEvents
        );
    }

    #[test]
    fn formats_watch_footer_countdown_and_paused_state() {
        let active = watch_footer(42, Some("12:04:17"), Some(Duration::from_secs(48)));
        assert!(active.contains("Last poll #42 at 12:04:17"));
        assert!(active.contains("Next fetch in 48.0s"));
        assert!(active.contains("████████"));

        let paused = watch_footer(42, Some("12:04:17"), None);
        assert!(paused.contains("Watch paused"));
        assert!(!paused.contains("Next fetch"));
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
        let narrow = rendered_view(&snapshots, 0, 40);
        assert!(narrow.contains("Minimum"));

        let rendered = rendered_view(&snapshots, 0, 80);
        assert!(rendered.lines().all(|line| line.chars().count() <= 80));
    }
}
