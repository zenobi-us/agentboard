---
title: Sources
---

# Sources

A Source reads items from an external or local system. 

This CLI doc only covers how Sources fit into a Workspace.

For Source-specific config and behavior, see the specific source docs.

## Workspace Shape

```toml
[[sources]]
id = "local"

[sources.source]
kind = "qmd"
# source-specific fields live here

[[sources.actions]]
uses = "agentboard/run-cmd"

[sources.actions.with]
cmd = "echo {{ item.id }}"
```

## Execution Order

A workspace can define multiple Sources, and they can be sources of the same or different kinds.

They are run concurrently, and when a source has finished retrieving items, its Actions are run sequentially for each item.

## Normalized Item Shape

After downloading or reading items from a source, agentboard normalizes them into a small model that is stored in the Workspace store.

They are stored in a common shape, so Actions can be written to work with any Source.

Every Source produces normalized items:

- `id`
- `title`
- `status`
- `url`
- `source_id`
- `source_kind`
- `raw`

`raw` keeps the original Source payload so the normalized model can stay small.

## Source Docs

- [QMD source](/sources/qmd)
- [Jira source](/sources/jira)

## CLI-Owned Behavior

The CLI validates Source ids before a Run:

- Source ids must be nonempty.
- Source ids must be unique within one Workspace.

Source crates own Source-specific validation and collection behavior.
