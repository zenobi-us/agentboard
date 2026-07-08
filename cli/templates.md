# Templates (/cli/templates)



# Templates [#templates]

Action inputs are MiniJinja templates.

```toml
[sources.actions.with]
cmd = "echo {{ item.id }}: {{ item.title }}"
```

## Context [#context]

Templates can read:

* `workspace`
* `source`
* `item`
* `action`

Example values:

```jinja
{{ workspace.id }}
{{ workspace.path }}
{{ source.id }}
{{ item.id }}
{{ item.title }}
{{ item.status }}
{{ item.url }}
{{ action.uses }}
{{ action.index }}
```

`item.raw` contains the original Source payload:

```jinja
{{ item.raw.jira.fields.priority.name }}
```

## Filters [#filters]

AgentBoard registers `slugify` for conservative path and branch names:

```jinja
{{ item.title | slugify }}
```

Example:

```text
"Fix Login!" -> "fix-login"
```

## Expansion order [#expansion-order]

AgentBoard renders MiniJinja first, then expands configured path variables:

* leading `~/`
* `$VAR`
* `${VAR}`

Example:

```toml
root = "$WORKTREE_ROOT/{{ item.id | slugify }}"
```

## Action hash [#action-hash]

AgentBoard hashes the rendered Action inputs. That hash is part of retry identity:

```text
(source_id, item.id, source_action_index, rendered_action_hash)
```

If a template renders different inputs, AgentBoard treats it as new work.

## Examples [#examples]

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
