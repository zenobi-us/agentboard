# Context Map

## AgentBoard flow

```text
workspace config
      |
      v
config loader -> source adapters -> local store -> action runner
      |              |                 |             |
      v              v                 v             v
 TOML/YAML     jira/linear/etc     item cache   sync/worktree/cmd
```

## Boundaries

- Config loader parses and validates workspaces.
- Source adapters fetch raw items and normalize minimum fields.
- Store persists normalized items, raw payloads, and action results.
- Action runner renders MiniJinja templates and executes built-ins.
- Docs describe config and supported actions.

## Current scaffold

- `pkgs/crates/agentboard`: CLI crate placeholder and vision.
- `apps/docs`: public docs scaffold.
- `.github`, `.moon`, `proto`: copied monorepo infrastructure.
