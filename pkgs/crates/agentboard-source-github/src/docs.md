---
title: GitHub source
---

# GitHub source

Use `kind = "github"` with `mode = "issue"` to collect GitHub issues through GitHub issue search.

```toml
[[sources]]
id = "github-ready"

[sources.source]
kind = "github"
mode = "issue"
query = "repo:zenobi-us/agentboard is:open label:ready"
limit = 50
status_map = { ready = "ready", doing = "in-progress" }

[sources.source.credentials]
helper = "gh auth token"
```

Issue mode injects `is:issue` when it is missing so GitHub pull requests do not become AgentBoard Items.

## Identity and status

Issue mode item ids default to `owner/repo#number`. Status comes from the first matching `status_map` entry on the issue labels. If no label matches, status falls back to the GitHub issue state.

Use `field_map` only when the default normalized fields are wrong for a workspace.

```toml
[sources.source.field_map]
title = "title"
url = "html_url"
```

## Credentials

The credential helper is any command that writes a token to stdout. `gh auth token` is the common human-friendly helper, but any secret manager command can be used.
