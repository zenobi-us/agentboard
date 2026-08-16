---
title: Jira source
---

# Jira source

Use `kind = "jira"` to collect Jira issues with JQL.

```toml
[[sources]]
id = "jira-ready"

[sources.source]
kind = "jira"
site = "https://example.atlassian.net"
jql = "project = AB AND statusCategory = Todo"
limit = 50
```

Credentials come from environment variables by default. Defaults are `JIRA_EMAIL`
and `JIRA_API_TOKEN`.

```toml
[sources.source]
kind = "jira"
site = "https://example.atlassian.net"
email_env = "JIRA_EMAIL"
token_env = "JIRA_API_TOKEN"
jql = "project = AB ORDER BY updated DESC"
```

Or configure a credential helper. AgentBoard writes Git-style request lines to
stdin (`protocol`, `host`) and reads either `username`/`password` or
`email`/`token` lines from stdout.

```toml
[sources.source]
kind = "jira"
site = "https://example.atlassian.net"
jql = "project = AB ORDER BY updated DESC"

[sources.source.credentials]
helper = "agentboard-jira-credentials"
```

## Field mapping

AgentBoard uses Jira's internal issue `id` as stable `item.id` inside the Jira
site's Item Store. The Jira issue key is the default `item.reference_id`.

Defaults map `reference_id` from `key`, `title` from `fields.summary`, `status`
from `fields.status.name`, and `url` to `{site}/browse/{key}`. Override mappings
when you need custom Jira fields. Use `status_map` to normalize Jira status names.

AgentBoard automatically requests Jira fields referenced by mapping paths that
start with `fields.`. For example, `fields.customfield_10010.value` requests
the top-level Jira field `customfield_10010`.

```toml
[sources.source.field_map]
id = "key"
title = "fields.summary"
status = "fields.status.name"
url = "fields.customfield_10010"
```

`field_map.id` changes `item.reference_id`; it does not change `item.id`.

Use `status_map` when Jira's status names should normalize to AgentBoard statuses.

```toml
status_map = { "To Do" = "ready", "In Progress" = "doing" }
```

Use `fields` only for extra Jira API fields you want preserved in the raw
payload but do not use in `field_map`.

```toml
fields = ["assignee"]
```
