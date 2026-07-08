# Quickstart (/quickstart)



# Quickstart [#quickstart]

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

[sources.source]
kind = "qmd"
collections = ["tasks"]
query = "intent: Find ready work items\nlex: status ready"

[[sources.actions]]
uses = "agentboard/run-cmd"

[sources.actions.with]
cmd = "echo {{ item.id }} {{ item.title }}"
```

Planned CLI shape:

```bash
agentboard run work
agentboard list work
```
