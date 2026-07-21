# AgentBoard CLI Context

The CLI crate is the public entrypoint for AgentBoard. It loads workspace config, coordinates runs, persists local store records, renders action inputs, and dispatches source/action crates.

## Language

**Workspace**:
A TOML config file that names sources and the actions to run for each source. A Workspace may be empty while being initialized before Sources are configured.
_Avoid_: Project config, repo config

**Workspace Initialization**:
Creation of a named, empty Workspace ready for Source configuration.
_Avoid_: Project initialization, repository initialization

**Run**:
One execution of the AgentBoard pipeline: load a workspace, read sources, update the store, and execute pending actions.
_Avoid_: Collect when referring to the public command

**Watch**:
A repeated run loop for one workspace.
_Avoid_: Daemon unless it is actually installed as a service

**Store**:
The local append-only record of item observations and action attempts for one workspace.
_Avoid_: Database, cache when precision matters

**Item Bucket**:
A Store partition for one stable item universe. For Jira, the item universe is keyed by the normalized Jira site URL because Jira issue keys are only unique inside one Jira organization.
_Avoid_: Cache shard, config hash

## Boundaries

- The CLI owns config loading, explicit built-in registration, validation orchestration, store paths, locking, runtime orchestration, and user commands.
- The CLI does not know source-specific query semantics or branch on Source kinds; it invokes registered Source behavior.
- The CLI does not implement action side effects or branch on Action identifiers; it invokes registered Action behavior.
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
