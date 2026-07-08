---
title: Troubleshooting
---

# Troubleshooting

## `workspace lock is held`

Another `run` or `watch` is active for the same Workspace.

Fix: stop the other process. Do not delete `run.lock` while a process is running.

## `required command qmd not found`

The Workspace has a QMD Source, but `qmd` is not on `PATH`.

Fix: install QMD or remove the QMD Source from the Workspace.

## `required command git not found`

The Workspace uses `agentboard/create-worktree`, but `git` is not on `PATH`.

Fix: install Git or remove that Action.

## Action keeps retrying

AgentBoard retries failed Actions until one succeeds for the same retry identity:

```text
(source_id, item.id, source_action_index, rendered_action_hash)
```

Fix the command, credentials, paths, or Source item data. Then run again.

## Worktree path already exists

`agentboard/create-worktree` fails when `root` exists but is not the intended worktree for the configured branch.

Fix: choose a different `root`, remove the bad directory, or repair the existing worktree.

## Environment variable did not expand

AgentBoard expands variables after template rendering:

- `$VAR`
- `${VAR}`
- leading `~/`

Check that the variable exists in the environment of the `agentboard` process, not just your interactive shell startup file.

## Jira credentials fail

For environment variables, check:

```bash
echo "$JIRA_EMAIL"
test -n "$JIRA_API_TOKEN"
```

For credential helpers, check that the helper prints one username key and one password key:

```text
username=you@example.com
password=api-token
```

Accepted username keys: `username`, `email`.
Accepted password keys: `password`, `token`.

## Bad TOML or unknown field

Workspace config denies unknown fields except under `[sources.actions.with]`.

Use the schema:

```bash
agentboard schema > agentboard.schema.json
```

## `list` shows stale items

The Store is append-only. `list` shows the latest observed item by id from Store files. If the Source no longer returns an item, old observations can still exist locally.

For now, inspect or remove the Workspace Store manually if you need a clean local view.
