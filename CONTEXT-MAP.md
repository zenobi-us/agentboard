# Context Map

This repo uses a multi-context domain-doc layout. Read this file first, then read only the `CONTEXT.md` and scoped ADR files relevant to the app/package being changed.

## Contexts

| Path | Context doc | ADR scope | Scope |
| --- | --- | --- | --- |
| `apps/cli` | `apps/cli/CONTEXT.md` | `.memory/docs/adr/apps/cli/` | CLI commands, config loading, built-in registration, runtime orchestration, and local store. |
| `pkgs/crates/clankpipe-core` | `pkgs/crates/clankpipe-core/CONTEXT.md` | `.memory/docs/adr/pkgs/crates/clankpipe-core/` | Shared model, config envelopes, registration contracts, action/result structs, and cross-package helpers. |
| `pkgs/crates/clankpipe-source-qmd` | `pkgs/crates/clankpipe-source-qmd/CONTEXT.md` | `.memory/docs/adr/pkgs/crates/clankpipe-source-qmd/` | QMD collection/query source adapter. |
| `pkgs/crates/clankpipe-source-jira` | `pkgs/crates/clankpipe-source-jira/CONTEXT.md` | `.memory/docs/adr/pkgs/crates/clankpipe-source-jira/` | Jira JQL/API source adapter. |
| `pkgs/crates/clankpipe-action-run-cmd` | `pkgs/crates/clankpipe-action-run-cmd/CONTEXT.md` | `.memory/docs/adr/pkgs/crates/clankpipe-action-run-cmd/` | Shell command action executor. |
| `pkgs/crates/clankpipe-action-worktree` | `pkgs/crates/clankpipe-action-worktree/CONTEXT.md` | `.memory/docs/adr/pkgs/crates/clankpipe-action-worktree/` | Git worktree action executor. |
| `apps/docs` | _none yet_ | _none yet_ | Docs app. Uses AgentBoard terms from the CLI/core contexts when documenting product behavior. |
| `pkgs/tools/deployment` | _none yet_ | _none yet_ | Release/deployment helper scripts. No separate domain language resolved yet. |
| `.github` | _none yet_ | `.memory/docs/adr/` | Release coordination, publish dispatch, and CI helper scripts. |

## System flow

```text
workspace config
      |
      v
apps/cli config loader
      |
      v
source package -> normalized item -> apps/cli store -> rendered action -> action package
      |              |                  |                 |             |
      v              v                  v                 v             v
 QMD/Jira/etc   agentboard-core     item/action JSONL  MiniJinja    cmd/worktree
```

## Boundaries

- `apps/cli` owns user commands, workspace loading, built-in registration, validation orchestration, store paths, locking, runtime orchestration, and template rendering.
- `agentboard-core` owns shared model/config/result types, Source and Action contracts, resolved Plugin configuration nodes, and tiny helpers.
- `clankpipe-source-*` packages own source-specific query semantics, collection, raw payload capture, and normalization into Items.
- `clankpipe-action-*` packages own one side effect each and consume already-rendered inputs.
- Docs describe config and supported behavior; they must not document planned adapters/actions as complete.

## ADR rules

- Read `.memory/docs/adr/` for system-wide decisions if files exist there.
- Read the scoped ADR directory listed above for the path being changed.
- If changing a seam between contexts, read both contexts and both scoped ADR directories.
- If a new decision applies to one crate, write its ADR under that crate's scoped ADR directory.
- If a new decision applies across multiple contexts, write it under `.memory/docs/adr/` and name the affected contexts in the ADR.

## Review rules

- Read relevant ADRs before review.
- Store every worktree review at `.memory/docs/agents/reviews/{ticket-id}.md`.
- Overwrite the current review for the same ticket.
- Keep historical review revisions in Git history.
- Do not write review artifacts to `docs/reviews/` or `.scratch/reviews/`.

