---
title: Run command action
---

# Run Command Action

Use `@clankpipe/action-run-cmd` to run a shell command for each collected item.

```toml
[[sources.actions]]
uses = "@clankpipe/action-run-cmd"

[sources.actions.with]
cmd = "echo {{ item.id }} {{ item.title }}"
```

ClankPipe runs the command through `sh -c`. Templates can use the collected
item and source fields.

The command receives these environment variables:

- `CLANKPIPE_WORKSPACE_ID`
- `CLANKPIPE_SOURCE_ID`
- `CLANKPIPE_ITEM_ID`

The old `AGENTBOARD_*` names remain available as aliases.

Set `cwd` when the command should run from a specific directory.

```toml
[sources.actions.with]
cwd = "/home/me/dev/myrepo"
cmd = "pi --prompt '{{ item.title }}'"
```

For asynchronous launch commands, set `healthcheck` to a shell probe that
reports readiness with exit status `0`. ClankPipe runs the probe immediately
after a successful launch, then polls until it passes or times out.

```toml
[sources.actions.with]
cwd = "worktrees/issue-{{ item.reference_id }}"
cmd = "npx --yes open-terminal ..."
healthcheck = "test -f .agent-ready"
healthcheck_interval = "1s"
healthcheck_timeout = "30s"
```

The interval defaults to `1s`; the timeout defaults to `30s`. Both accept
human-readable durations such as `250ms`, `5s`, or `2m`. The healthcheck uses
the same `cwd` and ClankPipe environment variables as `cmd`. A launch failure
skips the healthcheck. A timeout fails the action, so a later run retries it.
