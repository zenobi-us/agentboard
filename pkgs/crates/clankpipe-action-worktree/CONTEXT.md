# AgentBoard Worktree Action Context

`clankpipe-action-worktree` executes the built-in `agentboard/worktree` action.

## Language

**Worktree action**:
A built-in Action that ensures an item's configured root is a Git worktree on the requested branch, creating or safely switching it as needed.
_Avoid_: Create worktree action, Worktrunk action unless the action actually uses Worktrunk

**Managed worktree**:
A valid Git worktree at the configured root that the Worktree action may reuse or safely switch to the requested branch.
_Avoid_: Existing directory success; arbitrary directories are errors

## Boundaries

- This package uses plain `git worktree` so workspace configs stay portable.
- This package executes already-rendered inputs only.
- The CLI owns template rendering, action hashing, retry decisions, and action attempt persistence.
- Existing roots are only reusable when they are already on the requested branch.

## ADRs

Read `.memory/docs/adr/pkgs/crates/clankpipe-action-worktree/` before changing git/worktree behavior.
