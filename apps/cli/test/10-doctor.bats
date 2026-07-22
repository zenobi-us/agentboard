#!/usr/bin/env bats

load test_helper

setup() { setup_agentboard_test; }
teardown() { teardown_agentboard_test; }

@test "doctor validates config Store source command and actions" {
  write_item AB-1
  write_workspace "true"

  run "$AB" --color never doctor "$TMP/workspace.toml"

  [ "$status" -eq 0 ]
  [[ "$output" == *"ok config"* ]]
  [[ "$output" == *"ok store"* ]]
  [[ "$output" == *"source md reachable (1 fetched; limit 50; available unknown)"* ]]
  [[ "$output" == *"actions [1]"* ]]
  [[ "$output" == *"agentboard/run-cmd [ok]"* ]]
  [[ "$output" == *"ok command qmd"* ]]
}

@test "doctor reports independent source failures together" {
  cat > "$TMP/workspace.toml" <<'EOF'
[[sources]]
id = "one"
[sources.source]
kind = "qmd"
collections = ["one"]
query = "ready"

[[sources]]
id = "two"
[sources.source]
kind = "qmd"
collections = ["two"]
query = "ready"
EOF
  export QMD_QUERY_EXIT=9

  run "$AB" --color never doctor "$TMP/workspace.toml"

  [ "$status" -ne 0 ]
  [[ "$output" == *"fail source one"* ]]
  [[ "$output" == *"fail source two"* ]]
  [[ "$output" == *"2 check(s)"* ]]
}

# Registry loading now owns config rejection, so doctor must never start its checks.
@test "doctor rejects invalid Workspace config before diagnostics or side effects" {
  write_item AB-1
  cat > "$TMP/workspace.toml" <<'EOF'
[[sources]]
id = "md"
[sources.source]
kind = "qmd"
collections = ["test"]
query = "ready"

[[sources]]
id = "md"
[sources.source]
kind = "qmd"
collections = ["test"]
query = "ready"
EOF
  printf blocker > "$TMP/not-a-directory"
  export XDG_DATA_HOME="$TMP/not-a-directory"

  run "$AB" --color never doctor "$TMP/workspace.toml"

  [ "$status" -ne 0 ]
  [[ "$output" == *"duplicate source id md"* ]]
  [[ "$output" != *"doctor.start"* ]]
  [[ "$output" != *"source md reachable"* ]]
}

@test "doctor reports missing qmd sh and git commands" {
  write_item AB-1
  cat > "$TMP/workspace.toml" <<EOF
[[sources]]
id = "md"
[sources.source]
kind = "qmd"
collections = ["test"]
query = "ready"

[[sources.actions]]
uses = "agentboard/run-cmd"
[sources.actions.with]
cmd = "true"

[[sources.actions]]
uses = "agentboard/create-worktree"
[sources.actions.with]
repo = "$TMP/repo"
root = "$TMP/root"
branch = "test"
EOF

  PATH="$TMP/bin" run "$AB" --color never doctor "$TMP/workspace.toml"
  [ "$status" -ne 0 ]
  [[ "$output" == *"agentboard/run-cmd [fail: required command sh not found"* ]]
  [[ "$output" == *"agentboard/create-worktree [fail: required command git not found"* ]]

  mkdir -p "$TMP/empty-bin"
  PATH="$TMP/empty-bin" run "$AB" --color never doctor "$TMP/workspace.toml"
  [ "$status" -ne 0 ]
  [[ "$output" == *"fail source md:"* ]]
  [[ "$output" == *"fail command qmd: required command qmd not found"* ]]
}