# 0008 Use one GitHub source with explicit modes

## Status

Accepted

## Context

AgentBoard models a Source as a configured provider of task-like items plus source-owned query semantics (`pkgs/crates/clankpipe-core/CONTEXT.md`). Source adapters preserve raw provider payloads and normalize provider records into small Items (`apps/cli/CONTEXT.md`). Source configuration and runtime behavior are registered explicitly through the static Registry (`.memory/docs/adr/0010-use-explicit-static-source-and-action-registration.md`).

GitHub exposes issue data through the GraphQL issue search API, while GitHub Projects v2 uses GraphQL project item, field, and content-union APIs. Issue mode uses the `ISSUE_ADVANCED` search type so its query follows GitHub's advanced search syntax, including grouped `OR` expressions. Project mode adds custom fields/status and different auth requirements, so it is not the same collector as issue mode (`.memory/docs/agents/github-source-implications.md`).

## Decision

Use one `clankpipe-source-github` package and one `github` Source registration with explicit mode-specific config flattened under `sources.source`, matching the Jira source shape.

MVP mode is `issue`:

- query: GitHub issue search query
- item identity: repository plus issue number
- field overrides: `field_map`, matching Jira's config vocabulary
- normalized status: `status_map`, matching issue labels first and issue state as fallback
- credential path: generic credential helper that returns a token, allowing helpers such as `gh auth token`

Later `project` mode is justified by Project custom fields/status. It must stay a separate collector inside the GitHub package, not a vague shared query path.

## Consequences

- Shared GitHub auth/client/error handling lives in one package.
- Core and CLI get one GitHub source kind instead of separate issue/project source kinds.
- Issue mode must guard against GitHub Search returning pull requests by requiring or injecting `type:issue` and `state:open` into the GraphQL search query.
- Label-derived status is source policy, so the mapping must be explicit in workspace config.
- Project mode remains deferred until custom fields/status are needed.
