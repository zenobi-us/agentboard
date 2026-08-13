---
title: QMD source
---

# QMD source

Use `uses = "@agentboard/source-qmd"` to collect items from [QMD](https://github.com/tobi/qmd) collections.

```toml
[[sources]]
id = "local-ready"

[sources.source]
uses = "@agentboard/source-qmd"
collections = ["tasks"]
query = "intent: Find ready work items\nlex: status ready"
limit = 50
```

AgentBoard runs `qmd query --full --format json` and reads each result's raw
document body. Each matched note must have YAML frontmatter with string fields
for `id`, `title`, and `status`. `url` is optional; when absent AgentBoard uses
the QMD document reference.

The QMD document reference is `item.id`. The mapped frontmatter `id` is
`item.reference_id`, so moving reference mapping does not change Store or Action
identity.

## Field mapping

Use `map` when your frontmatter uses different names or nested fields.

```toml
[sources.source.map]
id = "agentboard.id"
title = "title"
status = "workflow.status"
url = "links.issue"
```

`map.id` changes `item.reference_id`; it does not change `item.id`.
