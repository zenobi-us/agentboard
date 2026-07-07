# AgentBoard Context

AgentBoard is a local automation bridge for agent work queues. It reads task-like items from sources, keeps local observations, and runs source-owned actions for pending work.

## Language

**Workspace**:
A TOML config file that names sources and the actions to run for each source.
_Avoid_: Project config, repo config

**Source**:
A configured provider of task-like items plus an optional query that selects which items match.
_Avoid_: Tracker, integration

**Item**:
A normalized local copy of one task-like record from a source.
_Avoid_: Ticket, issue, task when referring to the normalized AgentBoard record

**Store**:
The local append-only record of item observations and action attempts for one workspace.
_Avoid_: Database, cache when precision matters

**Action**:
A source-owned side effect that runs for a matching item when that item/action has no previous success record.
_Avoid_: Job, hook, plugin

**Run**:
One execution of the AgentBoard pipeline: load a workspace, read sources, update the store, and execute pending actions.
_Avoid_: Collect when referring to the public command

**Watch**:
A repeated run loop for one workspace.
_Avoid_: Daemon unless it is actually installed as a service

## Relationships

- A **Workspace** has one or more **Sources**.
- A **Source** produces zero or more **Items**.
- A **Source** owns zero or more **Actions**.
- A **Run** reads each **Source**, records **Items** in the **Store**, and executes pending **Actions**.
- A **Watch** performs repeated **Runs** for the same **Workspace**.

## Example dialogue

> **Dev:** "Should `agentboard collect work` be the public command?"
> **Domain expert:** "No. **Run** is the public workflow. Collection is only an internal stage of a **Run**."
>
> **Dev:** "Are actions shared by every source in a **Workspace**?"
> **Domain expert:** "No. An **Action** belongs to the **Source** that declares it."

## Flagged ambiguities

- "collect" was used as a public command and as an internal pipeline stage. Resolved: **Run** is the public command; collection is an internal stage.
- "sync" was used as an action. Resolved: storing item observations is core **Run** behavior, not an **Action**.
