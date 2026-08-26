# AgentBoard LLM Action Context

`agentboard-action-llm` runs a rendered prompt with Pi.

## Boundaries

- AgentBoard owns prompt rendering, action identity, retries, and persistence.
- The action owns prompt-file reading, optional Git worktree creation, Pi argv construction, and terminal launch.
- Terminal adapters use native command argv. They do not use `sh -c`.
- Herdr pane commands require the current `HERDR_WORKSPACE_ID`.
- Existing worktrees are reused only when the requested branch already matches.
