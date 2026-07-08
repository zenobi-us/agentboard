# AgentBoard Run Command Action Context

`agentboard-action-run-cmd` executes the built-in `agentboard/run-cmd` action.

## Language

**Run command action**:
A built-in Action that runs a shell command with AgentBoard environment variables.
_Avoid_: Script runner plugin

**Rendered inputs**:
The action `with` values after MiniJinja rendering and environment expansion by the CLI.
_Avoid_: Template config inside this crate

## Boundaries

- This crate executes already-rendered inputs only.
- The CLI owns template rendering, action hashing, retry decisions, and action attempt persistence.
- The action owns process execution, cwd handling, stdout/stderr capping, and exit-status translation into `ActionRun`.
- Do not add custom shell parsing; `sh -c` is the contract.

## ADRs

Read `.memory/docs/adr/pkgs/crates/agentboard-action-run-cmd/` before changing process execution semantics or action output behavior.
