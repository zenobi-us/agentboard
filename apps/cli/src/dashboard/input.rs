use agentboard_core::model::Workspace;
use crossterm::event::{KeyCode, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;

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

pub(super) fn clicked_watch(mouse: &MouseEvent, watch_area: Rect) -> bool {
    matches!(mouse.kind, MouseEventKind::Down(MouseButton::Left))
        && mouse.column >= watch_area.x
        && mouse.column < watch_area.right()
        && mouse.row >= watch_area.y
        && mouse.row < watch_area.bottom()
}

pub(super) fn clicked_source(
    mouse: &MouseEvent,
    tree_area: Rect,
    workspace: &Workspace,
) -> Option<usize> {
    if !matches!(mouse.kind, MouseEventKind::Down(MouseButton::Left))
        || mouse.column < tree_area.x
        || mouse.column >= tree_area.right()
        || mouse.row < tree_area.y
        || mouse.row >= tree_area.bottom()
    {
        return None;
    }

    let mut row = tree_area.y + 1;
    for (index, source) in workspace.sources.iter().enumerate() {
        let height = 2 + source.configured.actions.len() as u16;
        if mouse.row >= row && mouse.row < row.saturating_add(height) {
            return Some(index);
        }
        row = row.saturating_add(height);
    }
    None
}

pub(super) fn previous_source(selected: usize, source_count: usize) -> usize {
    selected.checked_sub(1).unwrap_or(source_count - 1)
}

pub(super) fn next_source(selected: usize, source_count: usize) -> usize {
    (selected + 1) % source_count
}
