#!/usr/bin/env bats

load test_helper

setup() {
  setup_agentboard_test
  git init -q "$TMP/repo"
  git -C "$TMP/repo" config user.name AgentBoard
  git -C "$TMP/repo" config user.email agentboard@example.invalid
  touch "$TMP/repo/README"
  git -C "$TMP/repo" add README
  git -C "$TMP/repo" commit -qm initial
  mkdir -p "$TMP/worktrees"
}

teardown() { teardown_agentboard_test; }

write_worktree_workspace() {
  local root="$1"
  local branch="$2"
  local nonce="${3:-}"
  cat > "$TMP/workspace.toml" <<EOF
[[sources]]
id = "md"
[sources.source]
kind = "qmd"
collections = ["test"]
query = "ready"

[[sources.actions]]
uses = "agentboard/create-worktree"
[sources.actions.with]
repo = "$TMP/repo"
root = "$root"
branch = "$branch"
nonce = "$nonce"
EOF
}

@test "create-worktree creates a new branch and worktree" {
  write_item AB-1
  write_worktree_workspace "$TMP/worktrees/ab-1" "ab-1"

  run "$AB" run "$TMP/workspace.toml"

  [ "$status" -eq 0 ]
  [ "$(git -C "$TMP/worktrees/ab-1" branch --show-current)" = "ab-1" ]
  git -C "$TMP/repo" worktree list --porcelain | grep -q "$TMP/worktrees/ab-1"
}

@test "create-worktree attaches an existing branch" {
  write_item AB-1
  git -C "$TMP/repo" branch existing
  write_worktree_workspace "$TMP/worktrees/existing" "existing"

  run "$AB" run "$TMP/workspace.toml"

  [ "$status" -eq 0 ]
  [ "$(git -C "$TMP/worktrees/existing" branch --show-current)" = "existing" ]
}

@test "create-worktree reuses the intended existing worktree when rendered inputs change" {
  write_item AB-1 "Item AB-1" ready
  write_worktree_workspace "$TMP/worktrees/ab-1" "ab-1" '{{ item.status }}'

  run "$AB" run "$TMP/workspace.toml"
  [ "$status" -eq 0 ]
  write_item AB-1 "Item AB-1" doing
  run "$AB" -v --color never run "$TMP/workspace.toml"

  [ "$status" -eq 0 ]
  [[ "$output" == *"reused $TMP/worktrees/ab-1"* ]]
  [ "$(wc -l < "$(actions_store_file)")" -eq 2 ]
}

@test "create-worktree rejects an existing path for the wrong branch" {
  write_item AB-1
  mkdir -p "$TMP/worktrees/wrong"
  write_worktree_workspace "$TMP/worktrees/wrong" "expected"

  run "$AB" --color never run "$TMP/workspace.toml"

  [ "$status" -ne 0 ]
  [[ "$output" == *"exists but is not worktree for branch expected"* ]]
}