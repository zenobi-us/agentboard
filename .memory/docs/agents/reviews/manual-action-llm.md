# Action LLM review

## Scope

Added `@agentboard/action-llm` in the `agentboard/action-llm` worktree.

The action accepts one rendered prompt or prompt file, passes the prompt to a configured harness with harness arguments, optionally creates or reuses a Git worktree, and launches direct, Zellij, Herdr, tmux, or generic terminal commands.

## Validation

- `bun run --cwd pkgs/packages/agentboard-action-llm typecheck`: passed.
- `bun test pkgs/packages/agentboard-action-llm/src apps/cli/src/services/runtime.test.ts`: passed; 41 tests, 119 assertions.

## Known limits

- Foreground waiting is reliable for direct Pi execution. Terminal backends report after their launch command returns.
- Herdr requires `HERDR_WORKSPACE_ID` and uses the current workspace for tab and pane launches.
- Existing worktrees are reused only when their registered branch matches the requested branch.
