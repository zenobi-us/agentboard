use crossterm::event::{KeyCode, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;

use crate::store::SourceSnapshot;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum DashboardCommand {
    Quit,
    Previous,
    Next,
    Ignore,
}

pub(super) fn dashboard_command(code: KeyCode, modifiers: KeyModifiers) -> DashboardCommand {
    match code {
        KeyCode::Char('q') => DashboardCommand::Quit,
        KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => DashboardCommand::Quit,
        KeyCode::Left | KeyCode::Char('h') => DashboardCommand::Previous,
        KeyCode::Right | KeyCode::Char('l') => DashboardCommand::Next,
        _ => DashboardCommand::Ignore,
    }
}

pub(super) fn clicked_source(
    mouse: &MouseEvent,
    tabs_area: Rect,
    snapshots: &[SourceSnapshot],
) -> Option<usize> {
    if !matches!(mouse.kind, MouseEventKind::Down(MouseButton::Left))
        || mouse.column < tabs_area.x
        || mouse.column >= tabs_area.right()
        || mouse.row < tabs_area.y
        || mouse.row >= tabs_area.bottom()
    {
        return None;
    }

    let mut x = tabs_area.x;
    for (index, snapshot) in snapshots.iter().enumerate() {
        let width = snapshot.source_id.chars().count() as u16;
        if mouse.column >= x && mouse.column < x.saturating_add(width) {
            return Some(index);
        }
        x = x.saturating_add(width + 1);
    }
    None
}

pub(super) fn previous_source(selected: usize, source_count: usize) -> usize {
    selected.checked_sub(1).unwrap_or(source_count - 1)
}

pub(super) fn next_source(selected: usize, source_count: usize) -> usize {
    (selected + 1) % source_count
}
