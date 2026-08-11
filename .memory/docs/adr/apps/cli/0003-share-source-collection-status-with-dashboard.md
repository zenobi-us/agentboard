# Share Source collection status through Store status files

AgentBoard writes the live collection status for each Source to a small status file that the Dashboard reads during its normal polling. We use status files instead of IPC because the `run` command and Dashboard already share the Workspace Store, and polling is sufficient for this display. A status file keeps the last result and time, while a `collecting` status without the Workspace run lock becomes `cancelled` after a crash. Collection status is operational metadata and does not define the authoritative Source Snapshot.
