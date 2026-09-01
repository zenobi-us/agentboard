# ClankPipe CLI Context

The CLI is the public entrypoint for ClankPipe. It loads Workspace configuration, coordinates Runs, persists Store records, renders Action inputs, and dispatches Source and Action packages.

## Language

**Workspace**:
Configuration that names Sources and the Actions to run for each Source. A Workspace can use an executable TypeScript or JavaScript file, or a serialized data file.
_Avoid_: Project config, repo config

**Plugin**:
A package descriptor that provides one Source or Action. Every Plugin declares runtime creation, a TypeBox schema, and a health check. The Plugin role selects the runtime interface.
_Avoid_: Configured Source, configured Action, runtime instance

**Plugin Package**:
An installed package marked with the `clankpipe-package` keyword. One Plugin Package provides one Plugin.
_Avoid_: Extension package, adapter package

**Executable Configuration**:
A TypeScript or JavaScript Workspace file that imports Plugin Descriptors and creates resolved configuration nodes.
_Avoid_: Scripted settings, dynamic config

**Data Configuration**:
A JSON, YAML, or TOML Workspace file. Each Source and Action uses a package name in its `uses` field.
_Avoid_: Static configuration

**Resolved Configuration**:
Workspace configuration with validated Plugin references and Plugin data. It is the input to Source collection and Action execution.
_Avoid_: Raw configuration, parsed settings

**Plugin Runtime**:
Workspace-scoped executable behavior created from one resolved Plugin configuration. A loaded Workspace owns one runtime for each configured Source and Action.
_Avoid_: Prepared Action, runtime factory

**Action Execution**:
The Item-scoped use of an Action runtime after ClankPipe renders the Action inputs. Each execution receives one Item and its rendered inputs.
_Avoid_: Action runtime creation, Workspace loading

**Workspace Initialization**:
Creation of a named, empty Workspace ready for Source configuration.
_Avoid_: Project initialization, repository initialization

**Run**:
One execution of the ClankPipe pipeline: load a workspace, read sources, update the store, and execute pending actions.
_Avoid_: Collect when referring to the public command

**Watch Mode**:
A persistent command mode that repeats or refreshes the selected operation until cancellation. A watched Run reuses one loaded Workspace for every cycle. Configuration changes require a command restart. Watched Store views refresh their current state. Dashboard Watch Mode is enabled by default and can be toggled by the user.
_Avoid_: Daemon unless it is actually installed as a service

**Dashboard**:
A read-only terminal view of stored Item and Action state for one Workspace. A Dashboard observes the Store without executing a Run.
_Avoid_: Monitor, control panel

**Action Plan Result**:
The current summary of an Item's configured Actions: `error` when a current Rendered Action identity has no success and its latest attempt failed, or when an Action cannot render; `success` when every current identity has succeeded or none are configured; and `pending` otherwise, including when the latest attempt was cancelled.
_Avoid_: Latest attempt, aggregate Action state, Dashboard Result

**Pipeline Execution**:
The persisted Workspace and Source scoped state for one Item and Action plan. It remains visible after the Item leaves the current Source Snapshot.
_Avoid_: Source status, external Item status

**Store**:
The local append-only record of Item observations, Source Snapshots, and Action attempts for one Workspace.
_Avoid_: Database, cache when precision matters

**Source Snapshot**:
The complete set of Items observed by one configured Source during its latest successful collection. A failed or cancelled collection does not replace the previous Source Snapshot.
_Avoid_: Membership list, query cache

**Source Collection Status**:
Each configured Source has its own shared collection status: `collecting`, `complete`, `failed`, or `cancelled`. `complete` means that the Source query returned successfully and the Snapshot was committed. `failed` means that the Source query returned an error and includes a short error message. `cancelled` means that the collection stopped before the Source query completed, including runtime cancellation or a stale `collecting` status after a crash. The status keeps the last result and its time for the Dashboard. A failed or cancelled collection keeps the previous Snapshot, or shows no Snapshot when none exists. Collection status does not define the current Source Snapshot.
_Avoid_: Fetch status, fetching state

**Item Bucket**:
A Store partition for one stable item universe. For Jira, the item universe is keyed by the normalized Jira site URL because Jira issue keys are only unique inside one Jira organization.
_Avoid_: Cache shard, config hash

## Boundaries

- The CLI owns config loading, Plugin discovery, registry resolution, validation orchestration, Store paths, locking, runtime orchestration, and user commands.
- The CLI does not know Source-specific query semantics or branch on Source package names; it invokes resolved Source behavior.
- The CLI does not implement Action side effects or branch on Action package names; it invokes resolved Action behavior. The CLI owns Pipeline Execution state and claim budgets.
- The CLI persists raw source payloads with normalized items so source schemas can evolve without bloating the core item model.

## Pipeline

```text
workspace config
      |
      v
load + validate -> collect items -> append store -> claim -> render action -> execute action
      |                 |              |             |          |                |
      v                 v              v             v          v                v
 apps/cli       source packages  apps/cli store  apps/cli  apps/cli      action packages
```

## ADRs

Read `.memory/docs/adr/apps/cli/` before changing CLI command names, store layout, runtime orchestration, or source/action dispatch.
