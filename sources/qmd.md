# QMD source (/sources/qmd)



# QMD source [#qmd-source]

Use `kind = "qmd"` to collect items from [QMD](https://github.com/tobi/qmd) collections.

```toml
[[sources]]
id = "local-ready"

[sources.source]
kind = "qmd"
collections = ["tasks"]
query = "intent: Find ready work items\nlex: status ready"
limit = 50
```

AgentBoard runs `qmd query`, then `qmd get --full` for each result. Each
matched note must have YAML frontmatter with string fields for `id`, `title`,
and `status`. `url` is optional; when absent AgentBoard uses the QMD document
reference.

## Field mapping [#field-mapping]

Use `map` when your frontmatter uses different names or nested fields.

```toml
[sources.source.map]
id = "agentboard.id"
title = "title"
status = "workflow.status"
url = "links.issue"
```
