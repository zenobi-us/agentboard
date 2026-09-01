---
title: Templates
---

# Templates

Action inputs are MiniJinja templates.

```toml
[sources.actions.with]
cmd = "echo {{ item.reference_id }}: {{ item.title }}"
```

## Context

Templates can read:

- `workspace`
- `source`
- `item`
- `action`
- `actions` (preceding named Actions only)

Example values:

```jinja
{{ workspace.id }}
{{ workspace.path }}
{{ source.id }}
{{ source.source.uses }}
{{ source.actions[0].uses }}
{{ item.id }}
{{ item.reference_id }}
{{ item.title }}
{{ item.status }}
{{ item.url }}
{{ action.uses }}
{{ action.index }}
{{ actions.issue_worktree.inputs.root }}
```

`source` is the complete configured Source. Adapter settings stay nested under
`source.source`, and configured Actions stay under `source.actions`.

An Action may declare a Source-scoped `id`. Later Actions can read that named
Action's final rendered inputs through `actions.<id>.inputs`. Unnamed Actions are
absent, and missing or forward references fail rendering for that Item.

```toml
[[sources.actions]]
id = "issue_worktree"
uses = "@clankpipe/action-worktree"
[sources.actions.with]
repo = "~/Projects/MyProject"
root = "$WORKTREE_ROOT/{{ item.id | slugify }}"
branch = "{{ item.id | slugify }}"

[[sources.actions]]
uses = "@clankpipe/action-run-cmd"
[sources.actions.with]
cwd = "{{ actions.issue_worktree.inputs.root }}"
cmd = "pwd"
```

Stored-success skips and dry runs still render each Action in order, so later
Actions receive freshly rendered named inputs. Action IDs do not change retry
identity.

Use `item.reference_id` for provider-facing names and messages. `item.id` is the
stable identity used by the Store and Action retry checks.

`item.raw` contains the original Source payload:

```jinja
{{ item.raw.jira.fields.priority.name }}
```

## Filters

ClankPipe registers `slugify` for conservative path and branch names:

```jinja
{{ item.title | slugify }}
```

Example:

```text
"Fix Login!" -> "fix-login"
```

## Rendering and expansion order

ClankPipe processes Action inputs in three stages:

1. MiniJinja renders every input.
2. ClankPipe expands leading `~/`, `$VAR`, and `${VAR}` only in path inputs:
   - `@clankpipe/action-run-cmd`: `cwd`
   - `@clankpipe/action-worktree`: `repo`, `root`
3. `@clankpipe/action-run-cmd` passes `cmd` and `healthcheck` to `sh -c`. The shell expands command variables after changing to `cwd`.

```toml
[[sources.actions]]
uses = "@clankpipe/action-worktree"
[sources.actions.with]
repo = "~/Projects/MyProject"
root = "$WORKTREE_ROOT/{{ item.id | slugify }}"
branch = "{{ item.id | slugify }}"

[[sources.actions]]
uses = "@clankpipe/action-run-cmd"
[sources.actions.with]
cwd = "$WORKTREE_ROOT/{{ item.id | slugify }}"
cmd = '''printf '%s: %s\n' "{{ item.reference_id }}" "$PWD"'''
```

Here ClankPipe expands `$WORKTREE_ROOT` in path fields, MiniJinja renders item fields, and the spawned shell resolves `$PWD` from its configured `cwd`.

## Action hash

ClankPipe hashes the final inputs after MiniJinja and ClankPipe-time path expansion. Those exact strings are passed to the Action, so shell variables in `cmd` remain literal in both the hash and the command given to `sh -c`. The hash is part of retry identity:

```text
(source_id, item.id, source_action_index, rendered_action_hash)
```

If a template renders different inputs, ClankPipe treats it as new work.

## Examples

Create one worktree per item:

```toml
[sources.actions.with]
repo = "~/Projects/MyProject"
root = "~/Projects/MyProject.worktrees/{{ item.id | slugify }}"
branch = "{{ item.id | slugify }}"
```

Open a terminal tab:

```toml
[sources.actions.with]
cmd = "zellij action new-tab --name {{ item.id | slugify }}"
cwd = "~/Projects/MyProject"
```
