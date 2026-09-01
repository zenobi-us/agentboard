# Ensure worktree desired state safely

`agentboard/worktree` replaces the pre-release `agentboard/create-worktree` ID without an alias and treats `repo`, `root`, and local `branch` as desired state. The Action creates an absent worktree, reuses an exact same-repository worktree already on the requested branch, or switches a clean managed worktree; missing branches always start from the configured repository's `HEAD`, while dirty worktrees, branch collisions, unrelated roots, and destructive force/reset behavior fail. This makes retries converge without moving or discarding local work.
