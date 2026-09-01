---
title: GitHub source
---

# GitHub source

Use `uses = "@clankpipe/source-github"` with `mode = "issue"` to collect GitHub issues through GitHub issue search.

```toml
[[sources]]
id = "github-ready"

[sources.source]
uses = "@clankpipe/source-github"
mode = "issue"
query = "repo:zenobi-us/agentboard is:open label:ready"
limit = 50

[sources.source.credentials]
helper = "gh auth token"

[sources.source.status_map]
ready = "ready"
```

`mode`, `query`, `credentials`, and a non-empty `status_map` are required. Issue mode sends `query` to GitHub GraphQL `ISSUE_ADVANCED` search and adds `state:open type:issue`. This supports GitHub advanced search syntax, including grouped `OR` expressions, while excluding pull requests.

`limit` defaults to 50. `field_map` is optional. Each `status_map` key matches an issue label or the GitHub state, and its value becomes the normalized Item status.

## Identity, reference, and status

Issue mode uses `owner/repo#number` as `item.id` and the issue number as
`item.reference_id`. This keeps equal issue numbers from different repositories
distinct while templates can use the normal GitHub issue number.

Status comes from the first matching `status_map` entry on the issue labels. If no label matches, status falls back to the GitHub issue state.

Use `field_map` only when the default normalized fields are wrong for a workspace.

```toml
[sources.source.field_map]
id = "custom.reference"
title = "title"
url = "html_url"
```

`field_map.id` changes `item.reference_id`; it does not change `item.id`. Issue mode keeps the REST field names `html_url` and `repository_url` available for field maps while it uses GraphQL internally.

Use `status_map` to normalize GitHub issue labels to workspace status values.

```toml
[sources.source.status_map]
"ready" = "ready"
"blocked" = "blocked"
"done" = "done"
```

## Credentials

The credential helper is any command that writes a token to stdout. `gh auth token` is the common human-friendly helper, but any secret manager command can be used.
