# Use cooperative cancellation through runtime contexts

AgentBoard uses one invocation-scoped cancellation signal, created by the CLI composition root and passed through runtime and health-check contexts. The Bun runtime uses `AbortSignal`.

The first Ctrl-C cancels active work and stops new work. The second Ctrl-C force-exits. Cancellation exits with status 130. Built-ins must terminate owned requests and process groups. AgentBoard does not promise rollback of external side effects. Interrupted Actions persist a `cancelled` outcome and remain pending. Cancelled Source collections commit no authoritative Source Snapshot.
