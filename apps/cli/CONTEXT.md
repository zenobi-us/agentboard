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

**Action Run**:
One immediate claim-and-action cycle for selected Sources. An Action Run retries failed, cancelled, and stale Items when they have no previous successful Action result. A Source cannot run another Action Run while one is active. A Workspace Action Run skips busy Sources. An Action Run does not change Watch Mode.
_Avoid_: Run when referring to a claim-and-action cycle

**Workspace Initialization**:
Creation of a named, empty Workspace ready for Source configuration.
_Avoid_: Project initialization, repository initialization

**Run**:
One execution of the ClankPipe pipeline: load a workspace, read sources, update the store, and execute pending actions.
_Avoid_: Collect when referring to the public command

**Source Polling**:
The automatic background operation in the TUI that repeats Source collection to find new Items. Each Source has one polling timer with a fixed interval of 60 seconds. A polling tick collects the Source, then claims Items only when Watch Mode is enabled for that Source. If the Source has an active Action Run, the tick updates the Snapshot and skips the Action Run. If collection fails, the Source keeps its previous Snapshot and Watch Mode can claim Items from that Snapshot. A forced fetch does not start while another fetch is active. Source Polling updates Source Snapshots and does not execute Actions when Watch Mode is disabled. Source Polling is always enabled in the TUI and cannot be toggled.
_Avoid_: Watch Mode when referring to background collection

**Watch Mode**:
The timed operation in the TUI that attempts to claim available Items and move them through the Action pipeline. Watch Mode is enabled separately for each Source. Workspace Watch Mode is an alias that enables Watch Mode for every Source. The user can toggle Watch Mode for one Source or for the whole Workspace. Workspace toggling sets every Source to the same value. If every Source is enabled, Workspace toggling disables every Source. Otherwise, it enables every Source. All Sources start with Watch Mode enabled in the TUI. Workspace totals sum each Source total and count one Item once per Source. Configuration changes require a command restart.
_Avoid_: Daemon unless it is actually installed as a service

**TUI**:
The interactive terminal view of stored Item and Action state for one Workspace. The TUI shows one navigable Workspace tree. Workspace, Source, Action, and Item details appear in drawers. The Workspace config opens in the user’s editor. The TUI observes the Store and can start Action Runs.
_Avoid_: Monitor, control panel

**Action Plan Result**:
The current summary of an Item's configured Actions: `error` when a current Rendered Action identity has no success and its latest attempt failed, or when an Action cannot render; `success` when every current identity has succeeded or none are configured; and `pending` otherwise, including when the latest attempt was cancelled.
_Avoid_: Latest attempt, aggregate Action state, TUI Result

**Pipeline Execution**:
The persisted Workspace and Source scoped state for one Item and Action plan. It remains visible after the Item leaves the current Source Snapshot.
_Avoid_: Source status, external Item status

**Available Item**:
An Item in the current Source Snapshot with no Pipeline Execution for the current Action plan. `Available` is a display group, not a Pipeline state. Failed, cancelled, and stale Items are not Available Items.
_Avoid_: Unclaimed Item when the current Action plan has a Pipeline Execution

**Claimed Item**:
An Item with a Pipeline Execution for the current Action plan. A Claimed Item appears in the `Claimed` display group, under the next Action. A `claimed` Pipeline Execution with no `action_index` appears under the first Action. A Claimed Item remains visible after it leaves the current Source Snapshot. When the Item remains in the Snapshot, the tree uses the latest Source data with the Pipeline state. Otherwise, the tree uses the last Pipeline Item data. The child state glyph shows every Pipeline state except `claimed`, because the display group already shows that state.
_Avoid_: In-progress Item when the Item has a terminal failure

**Force Claim**:
A request to claim an Available Item without counting it against `claim_limit`. Force Claim still follows the Action order and the previous-success rule.
_Avoid_: Force run when referring to an Item claim

**Suppressed Item**:
An Item that the user excludes from the Action pipeline. Suppression is keyed by Source ID and Item identity. It stays across Action plan changes. Suppression keeps the Source Snapshot and Action history. The Item does not appear under `available` or a claimed Action. The user can suppress any Item under an Action, including one with `succeeded` Pipeline state. An Item-level `r` action removes suppression and starts an Action Run. An Action-level `f` action bypasses the previous-success rule for that Action. If later Actions already succeeded, `y` reruns the selected Action and all later Actions. `n` runs only the selected Action. Otherwise, later Actions follow normal rules. Every confirmation accepts `Escape` to abort the request. `n` has request-specific meaning and does not replace `Escape`.
_Avoid_: Deleted Item when the Source record still exists

**No-Action Item**:
An Item from a Source with no configured Actions. A No-Action Item appears in a separate tree group and cannot enter the Action pipeline.
_Avoid_: Available Item when no Action is configured

**Item Event Log**:
The ordered history of Pipeline state changes and Action attempts for one Item.
_Avoid_: Workspace event log when referring to Item history

**Workspace Item Events**:
The combined Item Event Log entries for all Sources in one Workspace.

**Source Fetch Log**:
The complete ordered history of Source collection results. Each entry records the collection state, time, and error when collection fails.
_Avoid_: Item Event Log when referring to Source collection history

**Store**:
The local append-only record of Item observations, Source Snapshots, and Action attempts for one Workspace.
_Avoid_: Database, cache when precision matters

**Source Snapshot**:
The complete set of Items observed by one configured Source during its latest successful collection. A failed or cancelled collection does not replace the previous Source Snapshot.
_Avoid_: Membership list, query cache

**Source Collection Status**:
Each configured Source has its own shared collection status: `collecting`, `complete`, `failed`, or `cancelled`. `complete` means that the Source query returned successfully and the Snapshot was committed. `failed` means that the Source query returned an error and includes a short error message. `cancelled` means that the collection stopped before the Source query completed, including runtime cancellation or a stale `collecting` status after a crash. The status keeps the last result and its time for the TUI. The TUI Source tree shows this status. Source totals show separate counts for all, available, claimed, error, and no-action Items. A failed or cancelled collection keeps the previous Snapshot, or shows no Snapshot when none exists. Collection status does not define the current Source Snapshot.
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
