# AgentBoard Worktree Action Context

`agentboard-action-worktree` executes the built-in `agentboard/create-worktree` action.

## Language

**Worktree action**:
A built-in Action that creates or reuses a Git worktree for an item.
_Avoid_: Worktrunk action unless the action actually uses Worktrunk

**Reusable worktree**:
An existing root whose current branch matches the requested branch.
_Avoid_: Existing directory success; mismatched directories are errors

## Boundaries

- This crate uses plain `git worktree` so workspace configs stay portable.
- This crate executes already-rendered inputs only.
- The CLI owns template rendering, action hashing, retry decisions, and action attempt persistence.
- Existing roots are only reusable when they are already on the requested branch.

## ADRs

Read `.memory/docs/adr/pkgs/crates/agentboard-action-worktree/` before changing git/worktree behavior.
