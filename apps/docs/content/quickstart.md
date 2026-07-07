---
title: Quickstart
---

# Quickstart

Install tools and inspect projects:

```bash
proto install
bun install
moon query projects
```

Create a workspace file:

```text
~/.config/agentboard/work.toml
```

Example workspace:

```toml
[[sources]]
id = "local"
query = "status:ready"

[sources.source]
kind = "markdown"
path = "~/Projects/MyProject/tasks"

[[sources.actions]]
uses = "agentboard/sync"

[[sources.actions]]
uses = "agentboard/run-cmd"

[sources.actions.with]
cmd = "echo {{ item.id }} {{ item.title }}"
```

Planned CLI shape:

```bash
agentboard collect work
agentboard run work
agentboard list
```
