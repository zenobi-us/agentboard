---
title: LLM action
---

Use `@clankpipe/action-llm` to run a Pi prompt directly or in a terminal.

```toml
[[sources.actions]]
uses = "@clankpipe/action-llm"

[sources.actions.with]
prompt = "Implement {{ item.reference_id }}: {{ item.title }}"
mode = "foreground"

[sources.actions.with.terminal]
kind = "herdr"
container = "tab"
harness = "pi"
harness_args = ["--approve"]
cwd = "/home/me/dev/project"
```

Set exactly one of `prompt` or `prompt_file`. The prompt file path is read after
ClankPipe renders action inputs.

Use `worktree` to create or reuse a clean Git worktree before the agent starts.
The action does not switch an existing worktree to another branch.

```toml
[sources.actions.with.worktree]
repo = "/home/me/dev/project"
root = "/home/me/dev/project.trees/{{ item.id }}"
branch = "clankpipe/{{ item.id }}"
```

Set `terminal` to `zellij`, `herdr`, `tmux`, or `generic`. Set `harness` to the
agent harness executable and `harness_args` to its arguments. The rendered
prompt is the final harness argument. Pi starts a new saved session by default.
Configure `--session-id` or `--no-session` explicitly when you need different behavior. Direct launches wait for Pi in
`foreground` mode. Direct `background` launches return `running` immediately
and ClankPipe records the final result when the child exits.

Terminal launchers return `running` after they accept the command. Herdr
worktree launches create a new tab in the worktree workspace. Herdr, Zellij,
and tmux do not expose agent completion to this Action. ClankPipe
marks such executions `stale (disconnected)` during recovery unless a later
Action execution records completion.
