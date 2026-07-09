# GitHub Issues vs GitHub Projects source implications

## Recommendation

Build `agentboard-source-github` as **one crate with explicit modes**, and ship `mode = "issue"` first. Do not start with separate `agentboard-source-github-issues` and `agentboard-source-github-projects` crates.

Reason: Issues gives the shortest useful source adapter: repo/search-shaped data, REST pagination, small normalized `Item`, and raw issue payload storage. Projects is useful for board reality, but it adds GraphQL, `read:project`, custom field values, content unions, draft issues, pull requests, and redacted items before the first GitHub source is useful. AgentBoard already says source adapters own query semantics, raw payloads stay in `Item.raw`, source adapters stay isolated from actions, and action identity is source-owned (`.memory/docs/adr/pkgs/crates/agentboard-source-qmd/0007-source-adapters-own-query-semantics.md`, `pkgs/crates/agentboard-core/CONTEXT.md`, `AGENTS.md`, `.memory/docs/adr/pkgs/crates/agentboard-core/0005-actions-are-owned-by-sources.md`).

Minimum viable config shape:

```toml
[[sources]]
id = "github"

[sources.source]
kind = "github"
mode = "issue"
query = "repo:zenobi-us/agentboard is:open label:ready"
limit = 50
status_map = { ready = "ready" }

[sources.source.credentials]
helper = "gh auth token"
```

Add `mode = "project"` only when a workspace needs project custom fields, project status, draft issues, or cross-repo project membership.

## Existing AgentBoard fit

AgentBoard source kinds are currently explicit enum variants in `SourceKind`, and CLI collection dispatch matches on that enum, so any GitHub source needs both core config changes and CLI wiring (`pkgs/crates/agentboard-core/src/model.rs`, `apps/cli/src/adapters.rs`). The current Jira source already demonstrates the network-source pattern: provider query is passed through, requested fields are selected, provider JSON is normalized into `Item`, and the raw provider record is preserved (`pkgs/crates/agentboard-source-jira/src/lib.rs`, `pkgs/crates/agentboard-source-jira/CONTEXT.md`). The store records item observations and action attempts as append-only JSONL, with item buckets keyed by stable upstream item universes and action attempts scoped by source slug plus source hash (`.memory/docs/adr/apps/cli/0002-store-items-and-actions-as-per-source-jsonl.md`, `apps/cli/CONTEXT.md`).

```text
workspace source config
        |
        v
SourceKind::Github { mode }
        |
        v
GitHub API boundary -> normalized Item + raw payload -> store -> source-owned actions
        |                         |
        |                         +-- Item: id/title/status/url/source/raw
        +-- issue mode: repo issues or issue search
        +-- project mode: ProjectV2 items + fields + content union
```

## Comparison

| Area | GitHub Issues source | GitHub Projects source | AgentBoard implication |
| --- | --- | --- | --- |
| Data model fit | Repository issues expose stable issue numbers, titles, state, labels, assignees, milestones, timestamps, and URLs through REST issue endpoints; GitHub REST also notes repository issue listings return open issues by default (`GET /repos/{owner}/{repo}/issues`) (https://docs.github.com/en/rest/issues/issues). | Projects v2 exposes project items, fields, field values, and item content through GraphQL; project item content can be issue, pull request, or draft issue, and inaccessible content can be redacted in GitHub's Projects API guide (https://docs.github.com/en/issues/planning-and-tracking-with-projects/automating-your-project/using-the-api-to-manage-projects; https://docs.github.com/en/graphql/reference/projects). | Issues maps cleanly to current `Item { id, title, status, url, raw }`. Projects needs raw project item + raw field values + content-specific normalization. |
| Auth/scopes | REST Issues endpoints can be accessed with fine-grained tokens where the endpoint permits repository Issues or Pull requests read permissions; public resources can also be read without auth but at lower rate limit (https://docs.github.com/en/rest/issues/issues; https://docs.github.com/en/rest/using-the-rest-api/rate-limits-for-the-rest-api). | GitHub's Projects API guide says GraphQL project queries require `read:project`, and mutations require `project`; CLI examples tell users to run `gh auth login --scopes "project"` or refresh with `gh auth refresh -s project` (https://docs.github.com/en/issues/planning-and-tracking-with-projects/automating-your-project/using-the-api-to-manage-projects). | One GitHub credential helper/env path can serve both, but Project mode must fail clearly when `read:project` is missing. |
| API shape | REST `GET /repos/{owner}/{repo}/issues`, user/org issue lists, and REST issue/PR search are resource/list endpoints with query parameters (https://docs.github.com/en/rest/issues/issues; https://docs.github.com/en/rest/search/search#search-issues-and-pull-requests). | Projects v2 is GraphQL-first for project items, fields, and field values; GitHub's own Projects API guide demonstrates GraphQL queries for `ProjectV2.items`, `fieldValues`, and `content` branches (https://docs.github.com/en/issues/planning-and-tracking-with-projects/automating-your-project/using-the-api-to-manage-projects). | Shared crate is useful for auth/client/errors, but modes need separate collectors. Forcing one generic GitHub query path would become junk. |
| Pagination/rate limits | REST pagination uses `per_page` and `page` plus the `Link` response header, with `per_page` max 100 where supported; REST primary rate limits are documented separately and authenticated users commonly get 5,000 requests/hour while unauthenticated requests are lower (https://docs.github.com/en/rest/using-the-rest-api/using-pagination-in-the-rest-api; https://docs.github.com/en/rest/using-the-rest-api/rate-limits-for-the-rest-api). | GraphQL requires cursor pagination with `first`/`last` between 1 and 100 and `pageInfo` cursors; GitHub documents GraphQL primary rate limits and point calculation separately (https://docs.github.com/en/graphql/guides/using-pagination-in-the-graphql-api; https://docs.github.com/en/graphql/overview/rate-limits-and-query-limits-for-the-graphql-api). | Issue mode can stop after `limit` items and simple Link walking. Project mode must budget query cost and page nested connections carefully. |
| Filtering/query support | REST repository issues support filters such as state, labels, assignee, milestone, sort, direction, since, and `issue_field_values`; REST search supports issue/PR search query strings (https://docs.github.com/en/rest/issues/issues; https://docs.github.com/en/rest/search/search#search-issues-and-pull-requests). GitHub CLI mirrors practical issue filtering via `gh issue list --assignee --author --label --mention --milestone --search --state --json --repo` (https://cli.github.com/manual/gh_issue_list). | GitHub CLI exposes `gh project item-list --query` for Projects filter syntax and examples such as `assignee:@me is:issue is:open` and `label:bug -status:Done`; the underlying project data still comes from Projects APIs (https://cli.github.com/manual/gh_project_item-list; https://docs.github.com/en/issues/planning-and-tracking-with-projects/automating-your-project/using-the-api-to-manage-projects). | Keep query semantics owned by GitHub modes. Issue mode may accept `search` or structured repo filters. Project mode should accept project filter syntax or GraphQL-oriented config, not reuse issue search blindly. |
| Webhooks/events | GitHub documents `issues` webhook events and payloads for issue activity (https://docs.github.com/en/webhooks/webhook-events-and-payloads). | GitHub documents Projects-related webhook events such as project v2 item changes on the same webhook payload reference (https://docs.github.com/en/webhooks/webhook-events-and-payloads). | Webhooks are not needed for the current run/watch poll model. If added later, keep them in a separate ingestion path; do not complicate MVP collection. |
| Local normalization | Issue defaults are obvious: `id` can be `repository#number` or node id, `title` from issue title, `status` from issue state, `url` from `html_url`, `raw.github.issue` from the issue object (https://docs.github.com/en/rest/issues/issues; `pkgs/crates/agentboard-core/src/model.rs`). | Project defaults need content-aware branching: issue/PR content has repository URLs and numbers, draft issue content does not have a repository issue identity, and redacted content may not provide usable display data (https://docs.github.com/en/issues/planning-and-tracking-with-projects/automating-your-project/using-the-api-to-manage-projects; https://docs.github.com/en/graphql/reference/projects). | Preserve raw provider payload either way. Project mode probably needs normalized status from a named Project field, not from issue state. |
| Implementation complexity | Low: add `Github` mode enum, crate/Cargo wiring, token lookup, REST client, pagination, duplicate id check, normalizer, tests. This follows Jira's existing async adapter shape (`pkgs/crates/agentboard-source-jira/src/lib.rs`, `.memory/docs/adr/pkgs/crates/agentboard-source-jira/0004-use-async-source-adapters.md`). | Medium/high: GraphQL client/query text, owner/project number lookup, item pagination, nested field value pagination, content union handling, redacted handling, custom field mapping, query-cost awareness (https://docs.github.com/en/issues/planning-and-tracking-with-projects/automating-your-project/using-the-api-to-manage-projects; https://docs.github.com/en/graphql/overview/rate-limits-and-query-limits-for-the-graphql-api). | Ship issue mode first. Project mode is a second collector inside the same crate, not a reason to delay all GitHub support. |
| Package boundary | Separate issue package would be small but would duplicate later GitHub auth/client/rate-limit code. | Separate project package would isolate complexity but force users into separate source/action histories for one GitHub work universe. | Use `pkgs/crates/agentboard-source-github` with explicit `Issue` and later `Project` config variants. Add one `SourceKind::Github` variant in core and one CLI dispatch branch. |

## Tradeoffs

### One crate with modes

Pros: shared auth/client/rate-limit handling, one GitHub source kind, less duplicated code, and a clean future path for Projects. This matches the repo's boring explicit config preference while avoiding a crate-per-API split (`AGENTS.md`, `pkgs/crates/agentboard-core/CONTEXT.md`).

Cost: the nested config enum must stay strict. If the crate accepts vague mixed fields like `repo`, `project`, `query`, and `fields` all at once, it will become ambiguous. Use `mode = "issue"` and later `mode = "project"` with mode-specific required fields.

### Separate packages

Pros: smaller issue crate and hard isolation from Project GraphQL complexity.

Cost: duplicates GitHub credential/client behavior, creates two source kinds in core, adds two CLI dispatch paths, and makes action histories split by package/source even when users think of it as one GitHub source (`apps/cli/src/adapters.rs`, `.memory/docs/adr/pkgs/crates/agentboard-core/0005-actions-are-owned-by-sources.md`).

### Project-first

Pros: best representation of planning boards, custom statuses, draft issues, and cross-repository work.

Cost: too much machinery before first value. It also forces every user to grant `read:project` even if they only need repo issues (https://docs.github.com/en/issues/planning-and-tracking-with-projects/automating-your-project/using-the-api-to-manage-projects).

## Minimum viable implementation path

1. Add `agentboard-source-github` crate with dependencies similar to Jira: `agentboard-core`, `anyhow`, `reqwest`, `serde_json` (`pkgs/crates/agentboard-source-jira/Cargo.toml`).
2. Add one `SourceKind::Github { mode, query, credentials, limit, field_map, status_map }` in `agentboard-core`, with `GithubSourceMode::Issue` first (`pkgs/crates/agentboard-core/src/model.rs`).
3. Wire `SourceKind::Github` to `agentboard_source_github::collect_items` in CLI dispatch (`apps/cli/src/adapters.rs`).
4. Implement Issue mode using REST search when `search` is present; require users to include `is:issue` or inject it deliberately to avoid pull requests, because GitHub's search endpoint is for issues and pull requests together (https://docs.github.com/en/rest/search/search#search-issues-and-pull-requests).
5. Page with REST `Link` headers and `per_page=100` until `limit` is reached; preserve `raw.github.issue` and reject duplicate normalized ids (`https://docs.github.com/en/rest/using-the-rest-api/using-pagination-in-the-rest-api`, `pkgs/crates/agentboard-source-jira/src/lib.rs`).
6. Defer Project mode until needed. When added, use GraphQL cursor pagination, require `read:project`, store raw project item + field values + content, and normalize status from a configured project field (https://docs.github.com/en/graphql/guides/using-pagination-in-the-graphql-api; https://docs.github.com/en/issues/planning-and-tracking-with-projects/automating-your-project/using-the-api-to-manage-projects).

## Sources kept

- GitHub REST Issues docs — endpoint shape, issue filters, pagination parameters, permissions: https://docs.github.com/en/rest/issues/issues
- GitHub REST Search docs — issue/PR search endpoint and query semantics: https://docs.github.com/en/rest/search/search#search-issues-and-pull-requests
- GitHub REST pagination docs — Link header and `per_page`: https://docs.github.com/en/rest/using-the-rest-api/using-pagination-in-the-rest-api
- GitHub REST rate limit docs — REST primary and secondary limits: https://docs.github.com/en/rest/using-the-rest-api/rate-limits-for-the-rest-api
- GitHub Projects API guide — GraphQL Projects use, `read:project`, `project`, field values, content handling: https://docs.github.com/en/issues/planning-and-tracking-with-projects/automating-your-project/using-the-api-to-manage-projects
- GitHub GraphQL Projects reference — ProjectV2 schema category: https://docs.github.com/en/graphql/reference/projects
- GitHub GraphQL pagination docs — cursor pagination and page sizing: https://docs.github.com/en/graphql/guides/using-pagination-in-the-graphql-api
- GitHub GraphQL rate/query limits docs — point budgets and query limits: https://docs.github.com/en/graphql/overview/rate-limits-and-query-limits-for-the-graphql-api
- GitHub CLI `gh issue list` manual — practical issue filters and JSON output: https://cli.github.com/manual/gh_issue_list
- GitHub CLI `gh project item-list` manual — project item listing and Projects filter syntax: https://cli.github.com/manual/gh_project_item-list
- GitHub webhook payload docs — issues and Projects event availability: https://docs.github.com/en/webhooks/webhook-events-and-payloads
- AgentBoard repo docs/source — source boundaries, config enum, dispatch, storage, raw payload rules: `AGENTS.md`, `CONTEXT-MAP.md`, `pkgs/crates/agentboard-core/CONTEXT.md`, `apps/cli/CONTEXT.md`, `pkgs/crates/agentboard-source-jira/CONTEXT.md`, `pkgs/crates/agentboard-core/src/model.rs`, `apps/cli/src/adapters.rs`, `pkgs/crates/agentboard-source-jira/src/lib.rs`, `.memory/docs/adr/apps/cli/0002-store-items-and-actions-as-per-source-jsonl.md`, `.memory/docs/adr/pkgs/crates/agentboard-core/0005-actions-are-owned-by-sources.md`, `.memory/docs/adr/pkgs/crates/agentboard-source-qmd/0007-source-adapters-own-query-semantics.md`

## Sources dropped

- Linux distro/manpage mirrors of `gh` docs — duplicate of official GitHub CLI manual.
- Third-party blog/tutorial content — not needed; primary GitHub docs and repo source cover the decision.

## Grilling decisions

A follow-up domain-modeling grilling session resolved the current GitHub source direction into ADR 0008 (`.memory/docs/adr/0008-use-one-github-source-with-explicit-modes.md`):

- Use one `agentboard-source-github` package with explicit modes, not separate issue/project packages.
- Ship issue mode first using GitHub issue search query semantics.
- Use repository plus issue number as the stable item identity.
- Derive normalized status from an explicit configured label mapping.
- Use a generic credential helper returning a token, so `gh auth token` is just one possible helper.
- Treat Project mode as justified by Project custom fields/status, not by generic GitHub support.

## Gaps

- I did not verify a live GraphQL introspection response against this repo's token. Before implementing Project mode, run a small `gh api graphql` query against a real project to confirm the exact fields needed for the selected normalization.
- GitHub issue fields (`issue_field_values`) are newer and repository/organization availability depends on GitHub's documented constraints; test it against the target org before treating it as a substitute for Projects custom fields (https://docs.github.com/en/rest/issues/issues).
