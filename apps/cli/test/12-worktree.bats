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

# Emits only registered worktree inputs so strict typed validation stays meaningful.
write_worktree_workspace() {
  local root="$1"
  local branch="$2"
  cat > "$TMP/workspace.toml" <<EOF
[[sources]]
id = "md"
[sources.source]
kind = "qmd"
collections = ["test"]
query = "ready"

[[sources.actions]]
uses = "agentboard/worktree"
[sources.actions.with]
repo = "$TMP/repo"
root = "$root"
branch = "$branch"
EOF
}

@test "worktree expands repository and root environment paths" {
  write_item AB-1
  export REPO_PATH="$TMP/repo"
  export WORKTREE_ROOT="$TMP/worktrees"
  cat > "$TMP/workspace.toml" <<'EOF'
[[sources]]
id = "md"
[sources.source]
kind = "qmd"
collections = ["test"]
query = "ready"

[[sources.actions]]
uses = "agentboard/worktree"
[sources.actions.with]
repo = "$REPO_PATH"
root = "${WORKTREE_ROOT}/{{ item.reference_id }}"
branch = "{{ item.reference_id }}"
EOF

  run "$AB" run "$TMP/workspace.toml"

  [ "$status" -eq 0 ]
  [ "$(git -C "$TMP/worktrees/AB-1" branch --show-current)" = "AB-1" ]
}

@test "worktree creates a new branch and worktree" {
  write_item AB-1
  write_worktree_workspace "$TMP/worktrees/ab-1" "ab-1"

  run "$AB" run "$TMP/workspace.toml"

  [ "$status" -eq 0 ]
  [ "$(git -C "$TMP/worktrees/ab-1" branch --show-current)" = "ab-1" ]
  git -C "$TMP/repo" worktree list --porcelain | grep -q "$TMP/worktrees/ab-1"
}

@test "worktree attaches an existing branch" {
  write_item AB-1
  git -C "$TMP/repo" branch existing
  write_worktree_workspace "$TMP/worktrees/existing" "existing"

  run "$AB" run "$TMP/workspace.toml"

  [ "$status" -eq 0 ]
  [ "$(git -C "$TMP/worktrees/existing" branch --show-current)" = "existing" ]
}

# Deleting the attempt log forces a retry without inventing an unregistered nonce input.
@test "worktree reuses the intended existing worktree on a retried action" {
  write_item AB-1 "Item AB-1" ready
  write_worktree_workspace "$TMP/worktrees/ab-1" "ab-1"

  run "$AB" run "$TMP/workspace.toml"
  [ "$status" -eq 0 ]
  rm "$(actions_store_file)"
  run "$AB" -v --color never run "$TMP/workspace.toml"

  [ "$status" -eq 0 ]
  [[ "$output" == *"reused $TMP/worktrees/ab-1"* ]]
  [ "$(wc -l < "$(actions_store_file)")" -eq 1 ]
}

@test "worktree switches a clean managed worktree" {
  write_item AB-1
  git -C "$TMP/repo" branch target
  write_worktree_workspace "$TMP/worktrees/ab-1" "feature"
  run "$AB" run "$TMP/workspace.toml"
  [ "$status" -eq 0 ]

  write_worktree_workspace "$TMP/worktrees/ab-1" "target"
  run "$AB" run "$TMP/workspace.toml"

  [ "$status" -eq 0 ]
  [ "$(git -C "$TMP/worktrees/ab-1" branch --show-current)" = "target" ]
}

@test "worktree cancellation persists partial mutation and retries" {
  write_item AB-1
  mkdir -p "$TMP/hooks"
  cat > "$TMP/hooks/post-checkout" <<EOF
#!/bin/sh
echo started > "$TMP/hook-started"
sleep 10
EOF
  chmod +x "$TMP/hooks/post-checkout"
  git -C "$TMP/repo" config core.hooksPath "$TMP/hooks"
  write_worktree_workspace "$TMP/worktrees/ab-1" "ab-1"

  "$AB" --color never run "$TMP/workspace.toml" >"$TMP/run.out" 2>"$TMP/run.err" &
  RUN_PID=$!
  wait_for_pattern "$TMP/hook-started" started
  kill -INT "$RUN_PID"
  wait_status=0
  wait "$RUN_PID" || wait_status=$?

  [ "$wait_status" -eq 130 ]
  [ -d "$TMP/worktrees/ab-1" ]
  [ "$(git -C "$TMP/worktrees/ab-1" branch --show-current)" = "ab-1" ]
  /usr/bin/python3 - "$(actions_store_file)" <<'PY'
import json, pathlib, sys
records = [json.loads(line) for line in pathlib.Path(sys.argv[1]).read_text().splitlines()]
assert [record["outcome"] for record in records] == ["cancelled"]
PY

  git -C "$TMP/repo" config --unset core.hooksPath
  run "$AB" --color never run "$TMP/workspace.toml"
  [ "$status" -eq 0 ]
  /usr/bin/python3 - "$(actions_store_file)" <<'PY'
import json, pathlib, sys
records = [json.loads(line) for line in pathlib.Path(sys.argv[1]).read_text().splitlines()]
assert [record["outcome"] for record in records] == ["cancelled", "success"]
PY
}

@test "worktree rejects an arbitrary existing path" {
  write_item AB-1
  mkdir -p "$TMP/worktrees/wrong"
  write_worktree_workspace "$TMP/worktrees/wrong" "expected"

  run "$AB" --color never run "$TMP/workspace.toml"

  [ "$status" -ne 0 ]
  [[ "$output" == *"not a git repository"* ]]
}