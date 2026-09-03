# ClankPipe TUI

The TUI is an interactive terminal view for one Workspace.

## Main view

The TUI shows one focused Workspace tree:

```text
Workspace
  Source
    Available
      Item
    Claimed
      Action
        PipelineState Item
```

`Available` is a display group for Items with no Pipeline Execution.
`Claimed` is a display group for Items with any Pipeline Execution.
The state glyph shows every Pipeline state except `claimed`.

The tree includes current Source Snapshot Items and historical Pipeline Items.
A current Source Item uses the latest external status. A historical Item uses
its last Pipeline Item data.

Every visible node can receive focus. Arrow keys move through visible nodes.
Space opens or closes children. `i` opens details in a bottom drawer.
`Escape` closes the drawer and restores focus to the source node.

## Focused keymaps

- Any node: Space opens or closes children; `i` opens details.
- Workspace: `r` runs all idle Sources; `w` toggles every Source Watch Mode value; `e` opens Workspace config in the editor.
- Source: `f` fetches now; `r` runs one Action Run; `w` toggles Source Watch Mode.
- Available Item: `r` force claims after `y/n` confirmation.
- Claimed Action Item: `f` force runs the selected Action; `d` suppresses the Item after `y/n` confirmation.

`Escape` aborts every confirmation. `n` keeps request-specific meaning.

## Operation rules

Source Polling starts for every Source and repeats every 60 seconds. Each tick
fetches the Source. If Watch Mode is enabled, the tick then runs Actions. If an
Action Run is active, the tick updates the Snapshot and skips that Action Run.

A failed fetch keeps the previous Snapshot. Watch Mode can claim Items from that
Snapshot. A forced fetch does not start during another fetch.

A Source cannot run two Action Runs at the same time. Workspace `r` skips busy
Sources. Action-level `f` ignores the previous-success rule for the selected
Action. It asks before rerunning successful later Actions.

The TUI aborts active fetches and Actions when the user presses `Ctrl+Q`.
The Store records interrupted work as `cancelled` or `stale`.

## Details drawers

- Workspace: path, last update, totals, and all Item Event Log entries.
- Source: ID, query, arguments, totals, Items, and Source Fetch Log.
- Action: ID, arguments, and Items processed by that Action.
- Item: ID, title, timestamps, remote URI, content body, and Item Event Log.

The Workspace config opens in the user’s editor. Groups have no details drawer.
