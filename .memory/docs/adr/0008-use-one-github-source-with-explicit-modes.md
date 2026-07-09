# 0008 Use one GitHub source with explicit modes

## Status

Accepted

## Context

AgentBoard models a Source as a configured provider of task-like items plus source-owned query semantics (`pkgs/crates/agentboard-core/CONTEXT.md`). Source adapters preserve raw provider payloads and normalize provider records into small Items (`apps/cli/CONTEXT.md`). Existing dispatch is explicit by `SourceKind` in core and CLI (`pkgs/crates/agentboard-core/src/model.rs`, `apps/cli/src/adapters.rs`).

GitHub exposes issue data through REST issue/search APIs, while GitHub Projects v2 uses GraphQL project item, field, and content-union APIs. Project mode adds custom fields/status and different auth requirements, so it is not the same collector as issue mode (`.memory/docs/agents/github-source-implications.md`).

## Decision

Use one `agentboard-source-github` package and one `SourceKind::Github` with explicit nested mode-specific config.

MVP mode is `issue`:

- query: GitHub issue search query
- item identity: repository plus issue number
- normalized status: configured label mapping
- credential path: generic credential helper that returns a token, allowing helpers such as `gh auth token`

Later `project` mode is justified by Project custom fields/status. It must stay a separate collector inside the GitHub package, not a vague shared query path.

## Consequences

- Shared GitHub auth/client/error handling lives in one package.
- Core and CLI get one GitHub source kind instead of separate issue/project source kinds.
- Issue mode must guard against GitHub Search returning pull requests by requiring or injecting `is:issue`.
- Label-derived status is source policy, so the mapping must be explicit in workspace config.
- Project mode remains deferred until custom fields/status are needed.
