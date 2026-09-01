---
title: QMD source
---

# QMD source

Use `uses = "@clankpipe/source-qmd"` to collect items from [QMD](https://github.com/tobi/qmd) collections.

```toml
[[sources]]
id = "local-ready"

[sources.source]
uses = "@clankpipe/source-qmd"
collections = ["tasks"]
query = "intent: Find ready work items\nlex: status ready"
limit = 50
```

ClankPipe runs `qmd query --format json --full -n <limit>` and reads each result's raw
document body. It passes each configured collection with `-c <collection>`. Each matched note must have YAML frontmatter with string values at the paths configured by `map` for `id`, `title`, and `status`. `url` is optional; when absent ClankPipe uses the QMD document reference.

`collections` and `query` are required. `limit` defaults to 50 and must be positive. `map` is optional and defaults to the paths `id`, `title`, `status`, and `url`.

The QMD document reference is `item.id`. The mapped frontmatter `id` is
`item.reference_id`, so moving reference mapping does not change Store or Action
identity.

## Field mapping

Use `map` when your frontmatter uses different names or nested fields.

```toml
[sources.source.map]
id = "clankpipe.id"
title = "title"
status = "workflow.status"
url = "links.issue"
```

`map.id` changes `item.reference_id`; it does not change `item.id`.
