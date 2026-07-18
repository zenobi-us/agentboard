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

Example values:

```jinja
{{ workspace.id }}
{{ workspace.path }}
{{ source.id }}
{{ source.source.kind }}
{{ source.actions[0].uses }}
{{ item.id }}
{{ item.reference_id }}
{{ item.title }}
{{ item.status }}
{{ item.url }}
{{ action.uses }}
{{ action.index }}
```

`source` is the complete configured Source. Adapter settings stay nested under
`source.source`, and configured Actions stay under `source.actions`.

Use `item.reference_id` for provider-facing names and messages. `item.id` is the
stable identity used by the Store and Action retry checks.

`item.raw` contains the original Source payload:

```jinja
{{ item.raw.jira.fields.priority.name }}
```

## Filters

AgentBoard registers `slugify` for conservative path and branch names:

```jinja
{{ item.title | slugify }}
```

Example:

```text
"Fix Login!" -> "fix-login"
```

## Expansion order

AgentBoard renders MiniJinja first, then expands configured path variables:

- leading `~/`
- `$VAR`
- `${VAR}`

Example:

```toml
root = "$WORKTREE_ROOT/{{ item.id | slugify }}"
```

## Action hash

AgentBoard hashes the rendered Action inputs. That hash is part of retry identity:

```text
(source_id, item.id, source_action_index, rendered_action_hash)
```

If a template renders different inputs, AgentBoard treats it as new work.

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
