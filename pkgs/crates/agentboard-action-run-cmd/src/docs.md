---
title: Run command action
---

# Run Command Action

Use `agentboard/run-cmd` to run a shell command for each collected item.

```toml
[[sources.actions]]
uses = "agentboard/run-cmd"

[sources.actions.with]
cmd = "echo {{ item.id }} {{ item.title }}"
```

AgentBoard runs the command through `sh -c`. Templates can use the collected
item and source fields.

The command receives these environment variables:

- `AGENTBOARD_WORKSPACE_ID`
- `AGENTBOARD_SOURCE_ID`
- `AGENTBOARD_ITEM_ID`

Set `cwd` when the command should run from a specific directory.

```toml
[sources.actions.with]
cwd = "/home/me/dev/myrepo"
cmd = "pi --prompt '{{ item.title }}'"
```

For asynchronous launch commands, set `healthcheck` to a shell probe that
reports readiness with exit status `0`. AgentBoard runs the probe immediately
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
the same `cwd` and AgentBoard environment variables as `cmd`. A launch failure
skips the healthcheck. A timeout fails the action, so a later run retries it.
