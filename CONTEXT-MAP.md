# Context Map

This repo uses a multi-context domain-doc layout. Read this file first, then read only the `CONTEXT.md` and scoped ADR files relevant to the app/package being changed.

## Contexts

| Path | Context doc | ADR scope | Scope |
| --- | --- | --- | --- |
| `apps/cli` | `apps/cli/CONTEXT.md` | `.memory/docs/adr/apps/cli/` | CLI commands, config loading, runtime orchestration, local store, dispatch. |
| `pkgs/crates/agentboard-core` | `pkgs/crates/agentboard-core/CONTEXT.md` | `.memory/docs/adr/pkgs/crates/agentboard-core/` | Shared model, config types, action/result structs, cross-crate helpers. |
| `pkgs/crates/agentboard-source-qmd` | `pkgs/crates/agentboard-source-qmd/CONTEXT.md` | `.memory/docs/adr/pkgs/crates/agentboard-source-qmd/` | QMD collection/query source adapter. |
| `pkgs/crates/agentboard-source-jira` | `pkgs/crates/agentboard-source-jira/CONTEXT.md` | `.memory/docs/adr/pkgs/crates/agentboard-source-jira/` | Jira JQL/API source adapter. |
| `pkgs/crates/agentboard-action-run-cmd` | `pkgs/crates/agentboard-action-run-cmd/CONTEXT.md` | `.memory/docs/adr/pkgs/crates/agentboard-action-run-cmd/` | Shell command action executor. |
| `pkgs/crates/agentboard-action-worktree` | `pkgs/crates/agentboard-action-worktree/CONTEXT.md` | `.memory/docs/adr/pkgs/crates/agentboard-action-worktree/` | Git worktree action executor. |
| `apps/docs` | _none yet_ | _none yet_ | Docs app. Uses AgentBoard terms from the CLI/core contexts when documenting product behavior. |
| `pkgs/tools/deployment` | _none yet_ | _none yet_ | Release/deployment helper scripts. No separate domain language resolved yet. |

## System flow

```text
workspace config
      |
      v
apps/cli config loader
      |
      v
source crate -> normalized item -> apps/cli store -> rendered action -> action crate
      |              |                  |                 |             |
      v              v                  v                 v             v
 QMD/Jira/etc   agentboard-core     item/action JSONL  MiniJinja    cmd/worktree
```

## Boundaries

- `apps/cli` owns user commands, workspace loading, validation, store paths, locking, runtime orchestration, template rendering, and dispatch.
- `agentboard-core` owns shared model/config/result types and tiny helpers only.
- `agentboard-source-*` crates own source-specific query semantics, collection, raw payload capture, and normalization into Items.
- `agentboard-action-*` crates own one side effect each and consume already-rendered inputs.
- Docs describe config and supported behavior; they must not document planned adapters/actions as complete.

## ADR rules

- Read `.memory/docs/adr/` for system-wide decisions if files exist there.
- Read the scoped ADR directory listed above for the path being changed.
- If changing a seam between contexts, read both contexts and both scoped ADR directories.
- If a new decision applies to one crate, write its ADR under that crate's scoped ADR directory.
- If a new decision applies across multiple contexts, write it under `.memory/docs/adr/` and name the affected contexts in the ADR.
