# Context Map

## AgentBoard flow

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

- `pkgs/crates/agentboard`: CLI crate placeholder and vision.
- `apps/docs`: public docs scaffold.
- `.github`, `.moon`, `proto`: copied monorepo infrastructure.
