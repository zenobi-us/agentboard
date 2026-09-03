# Use independent Source Polling and Watch Mode in the TUI

## Status

Accepted

## Context

The TUI must show current Source status and ClankPipe pipeline progress in one
tree. Source collection must continue even when Action execution is off.
Users also need immediate Source and Item controls without global header buttons.

## Decision

The TUI MUST start Source Polling for every configured Source. Each Source
MUST use one 60-second timer. A timer MUST fetch the Source before it attempts an
Action Run. A failed fetch MUST keep the previous Source Snapshot. Watch Mode
MUST be able to claim Items from that Snapshot. An active Action Run MUST NOT
block Source Polling, but the timer MUST skip a second Action Run for that
Source.

All Sources MUST start with Watch Mode enabled. Source Watch Mode MUST be
independently toggleable. Workspace Watch Mode MUST be an alias for setting all
Source toggles to one value. If all Source toggles are enabled, Workspace
`w` disables all of them. Otherwise, it enables all of them.

The TUI MUST show one focused Workspace tree. `Available` MUST be a
display group for Items with no Pipeline Execution. `Claimed` MUST be a display
group for Items with any Pipeline Execution. Sources with no configured Actions
MUST show a separate `No Actions` group. Claimed Items MUST appear under the next
Action. The child state glyph MUST show every Pipeline state except
`claimed`. Claimed Items MUST remain visible after Source removal.

The tree MUST preserve focus and expansion by stable node identity after Source
Polling updates. Every visible node MUST receive focus. Arrow keys MUST move
focus through visible nodes. Space MUST open or close children. Details MUST
open in a full-width bottom drawer. Escape MUST close the drawer and restore
focus to the opening node.

The TUI MUST use these focused controls:

- Workspace: `r` runs all idle Sources; `w` sets all Source Watch Mode toggles; `e` opens Workspace config in the editor.
- Source: `f` fetches now; `r` runs one Action Run; `w` toggles Source Watch Mode.
- Available Item: `r` force claims after confirmation.
- Claimed Action Item: `f` force runs the selected Action; `d` suppresses the Item after confirmation.

A normal Action Run MUST retry failed, cancelled, and stale Items without a
previous successful Action result. A Force Claim MUST bypass only
`pipeline.claim_limit`. An Action-level force run MUST bypass the previous
success for that Action. If later Actions already succeeded, `y` reruns the
selected Action and all later Actions. `n` runs only the selected Action.

Suppression MUST use Source ID plus Item identity. It MUST keep Source Snapshot
and Action history. Item-level `r` MUST remove suppression and start an Action
Run. Every confirmation MUST accept Escape to abort. `n` MUST keep
request-specific meaning.

The Source drawer MUST include tabs for all, available, claimed, error, and fetch
log. The error tab MUST show Items with failed Action attempts. The fetch log MUST
contain all Source collection events, including successful, failed, and cancelled
events. Workspace and Item drawers MUST show their respective Item Event Log data.

## Consequences

The TUI is an operational view, not a read-only Store view. It can fetch
Sources and start Action Runs. The Store needs durable Source Fetch Log records
and durable suppression records. The TUI needs one owner for Source timers,
Action Runs, focus, expansion, and drawer state.

A failed Source can cause Watch Mode to use stale Source data. The TUI MUST show the Source collection status and time so users can see this condition.
