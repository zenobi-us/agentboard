ClankPipe                                                   v0.0.1

Workspace
  Source
    Available
      [1] item title · external-status
    Claimed
      create_worktree
        <state-glyph> [2] item title · external-status
      run_llm
        <state-glyph> [3] item title · external-status
        <state-glyph> [4] item title · external-status
    No Actions
      [5] item title · external-status

`Available` is a display group for Items with no Pipeline state.
`Claimed` is a display group for Items with any Pipeline state.
`No Actions` is a display group for Sources with no configured Actions.
The state glyph shows every Pipeline state except `claimed`.
A claimed Item with no action index appears under the first Action.

The tree is the only main view. Every visible node can receive focus.
Arrow keys move focus through visible nodes. Space opens or closes children.
Escape closes a details drawer and restores focus to its source node.

Focused node keymaps:

- any node: Space opens or closes children; `i` opens details
- Workspace: `r` runs all idle Sources; `w` toggles all Source Watch Mode values; `e` opens Workspace config in the editor
- Source: `f` fetches now; `r` runs one Action Run; `w` toggles Source Watch Mode
- Available Item: `r` force claims after `y/n` confirmation
- Claimed Action Item: `f` force runs the selected Action; `d` suppresses the Item after `y/n` confirmation

Source Polling runs every 60 seconds. All Sources start with Watch Mode enabled.
Source Polling fetches first. It runs Actions only when Watch Mode is enabled.

Details drawers use a full-width bottom anchor with two rows:

- Workspace: path, last update, totals; then all Workspace Item Events.
- Source: ID, query, arguments, totals; then tabs for all, available, claimed, error, and fetch log.
- Action: ID, arguments, and Item count; then Items processed by that Action.
- Item: ID, title, timestamps, and remote URI; then tabs for content body and event log.


