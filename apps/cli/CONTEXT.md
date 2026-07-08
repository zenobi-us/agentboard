# AgentBoard CLI Context

The CLI crate is the public entrypoint for AgentBoard. It loads workspace config, coordinates runs, persists local store records, renders action inputs, and dispatches source/action crates.

## Language

**Workspace**:
A TOML config file that names sources and the actions to run for each source.
_Avoid_: Project config, repo config

**Run**:
One execution of the AgentBoard pipeline: load a workspace, read sources, update the store, and execute pending actions.
_Avoid_: Collect when referring to the public command

**Watch**:
A repeated run loop for one workspace.
_Avoid_: Daemon unless it is actually installed as a service

**Store**:
The local append-only record of item observations and action attempts for one workspace.
_Avoid_: Database, cache when precision matters

## Boundaries

- The CLI owns config loading, validation, store paths, locking, runtime orchestration, and user commands.
- The CLI does not know source-specific query semantics beyond dispatch by source kind.
- The CLI does not implement action side effects; it dispatches configured actions to action crates.
- The CLI persists raw source payloads with normalized items so source schemas can evolve without bloating the core item model.

## Pipeline

```text
workspace config
      |
      v
load + validate -> collect items -> append store -> render action -> execute action
      |                 |              |              |                |
      v                 v              v              v                v
 apps/cli       source crates    apps/cli store   apps/cli      action crates
```

## ADRs

Read `.memory/docs/adr/apps/cli/` before changing CLI command names, store layout, runtime orchestration, or source/action dispatch.
