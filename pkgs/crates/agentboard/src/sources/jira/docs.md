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
fields = ["summary", "status"]
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

Defaults map `id` from `key`, `title` from `fields.summary`, `status` from
`fields.status.name`, and `url` to `{site}/browse/{key}`. Override mappings
when you need custom Jira fields.

```toml
[sources.source.map]
id = "key"
title = "fields.summary"
status = "fields.status.name"
url = "fields.customfield_10010"
```
