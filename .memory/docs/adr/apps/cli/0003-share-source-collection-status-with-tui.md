# Share Source collection status through Store status files

AgentBoard writes the current collection status for each Source to a small status
file that the TUI reads during Source Polling. We use Store files instead of
IPC because the `run` command and TUI already share the Workspace Store.

The current status is `collecting`, `complete`, `failed`, or `cancelled`. A
status file keeps the latest result and its time. A `collecting` status without
the Workspace run lock becomes `cancelled` after a crash. Collection status is
operational metadata. It does not define the authoritative Source Snapshot.

The Store also keeps a complete append-only Source Fetch Log. Each collection
attempt records its state, time, and error when collection fails. The TUI shows the current collection status on the Source node and the full
Fetch Log in the Source drawer.

Source Polling runs every 60 seconds for each TUI Source. It fetches the
Source even when Watch Mode is disabled. When Watch Mode is enabled, the same
tick attempts an Action Run after the fetch. A failed fetch keeps the previous
Snapshot. Watch Mode can claim Items from that Snapshot. An active Action Run
causes the tick to update the Snapshot and skip the Action Run.
