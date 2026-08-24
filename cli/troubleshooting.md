# Troubleshooting (/cli/troubleshooting)



# Troubleshooting [#troubleshooting]

## Item Store requires `reference_id` [#item-store-requires-reference_id]

The Item Store was written by an older AgentBoard version and cannot be loaded
by the current schema.

Fix: remove the affected `items-<source.slug>.jsonl` path named in the error,
then run the Workspace again. AgentBoard rebuilds Item observations from the
Source.

For Jira, QMD, and GitHub Sources with `field_map.id`, Item identities changed
with this schema. Existing successful Action records will not suppress Actions
for rebuilt Items. Review or temporarily disable configured Actions before
rebuilding if repeating them could cause duplicate side effects.

## `workspace lock is held` [#workspace-lock-is-held]

Another `run` or non-dry `run --watch` is active for the same Workspace.

Fix: stop the other process. Do not delete `run.lock` while a process is running.

## `required command qmd not found` [#required-command-qmd-not-found]

The Workspace has a QMD Source, but `qmd` is not on `PATH`.

Fix: install QMD or remove the QMD Source from the Workspace.

## `required command git not found` [#required-command-git-not-found]

The Workspace uses `@agentboard/action-worktree`, but `git` is not on `PATH`.

Fix: install Git or remove that Action.

## Action keeps retrying [#action-keeps-retrying]

AgentBoard retries failed Actions until one succeeds for the same retry identity:

```text
(source_id, item.id, source_action_index, rendered_action_hash)
```

Fix the command, credentials, paths, or Source item data. Then run again.

## Worktree Action refused an existing root [#worktree-action-refused-an-existing-root]

`@agentboard/action-worktree` only manages an exact worktree root from the configured `repo`. It reuses the requested branch, but refuses to switch dirty worktrees or a branch checked out in another worktree.

Fix the `repo` or `root`, commit or remove tracked and untracked changes, or release the requested branch from its other worktree. The Action never forces, resets, or cleans a worktree.

## Environment variable did not expand [#environment-variable-did-not-expand]

First identify where expansion belongs.

AgentBoard expands leading `~/`, `$VAR`, and `${VAR}` after MiniJinja rendering only for path inputs:

* `@agentboard/action-run-cmd.cwd`
* `@agentboard/action-worktree.repo`
* `@agentboard/action-worktree.root`

For these fields, check that the variable exists in the environment of the `agentboard` process, not only an interactive shell startup file.

Variables in `@agentboard/action-run-cmd.cmd` and `healthcheck` are left literal until `sh -c` runs. The shell starts in the configured `cwd`, so `$PWD` resolves there. Check shell syntax, quoting, exported variables, and whether the shell process receives the expected environment.

MiniJinja expressions such as `{{ item.reference_id }}` always render before either kind of environment expansion.

## Jira credentials fail [#jira-credentials-fail]

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

## Bad TOML or unknown field [#bad-toml-or-unknown-field]

Workspace config denies unknown fields, including inputs under `[sources.actions.with]` that the selected Action does not declare.

Use the schema:

```bash
agentboard schema > agentboard.schema.json
```

## `list` shows stale items [#list-shows-stale-items]

The Store is append-only. `list` shows the latest observed item by id from Store files. If the Source no longer returns an item, old observations can still exist locally.

For now, inspect or remove the Workspace Store manually if you need a clean local view.
