# Context Map

This repo uses a multi-context domain-doc layout. Read this file first, then read only the `CONTEXT.md` files relevant to the app/package being changed.

## Contexts

| Path | Context doc | Scope |
| --- | --- | --- |
| `apps/cli` | `apps/cli/CONTEXT.md` | Rust CLI domain: workspaces, sources, items, store, actions, runs, watch loops. |
| `pkgs/crates/agentboard-*` | `apps/cli/CONTEXT.md` | Shared CLI domain crates for models, sources, and actions. |
| `apps/docs` | _none yet_ | Docs app. Uses the CLI domain terms when documenting AgentBoard behavior. |
| `pkgs/tools/deployment` | _none yet_ | Release/deployment helper scripts. No separate domain language resolved yet. |

## System flow

```text
workspace config
      |
      v
config loader -> source adapters -> per-source store -> action runner
      |              |                    |               |
      v              v                    v               v
   TOML          markdown MVP       item/action JSONL  worktree/cmd
```

## Boundaries

- Config loader parses and validates TOML workspaces.
- Source adapters fetch raw items and normalize minimum fields.
- Store persists item observations, raw payloads, and action results in per-source JSONL files.
- Action runner renders MiniJinja templates and executes source-owned built-ins.
- Docs describe config and supported actions.

## Current scaffold

- `apps/cli`: CLI crate.
- `pkgs/crates/agentboard-*`: split Rust library crates for core, sources, and actions.
- `apps/docs`: public docs scaffold.
- `.github`, `.moon`, `proto`: copied monorepo infrastructure.
