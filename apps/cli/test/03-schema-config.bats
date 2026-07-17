#!/usr/bin/env bats

load test_helper

setup() { setup_agentboard_test; }
teardown() { teardown_agentboard_test; }

assert_bad_workspace() {
  local expected="$1"
  local body="$2"
  printf '%s\n' "$body" > "$TMP/bad.toml"
  run "$AB" run "$TMP/bad.toml"
  [ "$status" -ne 0 ]
  [[ "$output" == *"$expected"* ]]
}

@test "schema is valid JSON generated from WorkspaceConfig" {
  run "$AB" schema

  [ "$status" -eq 0 ]
  printf '%s' "$output" | /usr/bin/python3 -c 'import json,sys; value=json.load(sys.stdin); assert value["title"] == "WorkspaceConfig"; assert "sources" in value["properties"]'
}

@test "empty workspace is valid and runs with zero items" {
  printf 'sources = []\n' > "$TMP/empty.toml"

  run "$AB" --color never run "$TMP/empty.toml"

  [ "$status" -eq 0 ]
  [[ "$output" == *"0 items"* ]]
}

@test "TOML shape and source identity validation reject bad configs" {
  assert_bad_workspace "invalid array" 'sources = ['
  assert_bad_workspace "unknown field" $'sources = []\nunknown = true'
  assert_bad_workspace "source id cannot be empty" $'[[sources]]\nid = ""\n[sources.source]\nkind = "qmd"\ncollections = ["test"]\nquery = "ready"'
  assert_bad_workspace "duplicate source id md" $'[[sources]]\nid = "md"\n[sources.source]\nkind = "qmd"\ncollections = ["one"]\nquery = "ready"\n[[sources]]\nid = "md"\n[sources.source]\nkind = "qmd"\ncollections = ["two"]\nquery = "ready"'
}

@test "source-specific invariants reject invalid QMD Jira and GitHub configs" {
  assert_bad_workspace "requires at least one collection" $'[[sources]]\nid = "md"\n[sources.source]\nkind = "qmd"\ncollections = []\nquery = "ready"'
  assert_bad_workspace "requires query" $'[[sources]]\nid = "md"\n[sources.source]\nkind = "qmd"\ncollections = ["test"]\nquery = ""'
  assert_bad_workspace "limit must be greater than zero" $'[[sources]]\nid = "md"\n[sources.source]\nkind = "qmd"\ncollections = ["test"]\nquery = "ready"\nlimit = 0'
  assert_bad_workspace "requires site" $'[[sources]]\nid = "jira"\n[sources.source]\nkind = "jira"\nsite = ""\njql = "project = AB"'
  assert_bad_workspace "requires status_map" $'[[sources]]\nid = "github"\n[sources.source]\nkind = "github"\nmode = "issue"\nquery = "repo:owner/repo"\n[sources.source.credentials]\nhelper = "printf token"\n[sources.source.status_map]'
}

@test "action validation rejects unknown actions and missing required inputs" {
  assert_bad_workspace "unknown action example/nope" $'[[sources]]\nid = "md"\n[sources.source]\nkind = "qmd"\ncollections = ["test"]\nquery = "ready"\n[[sources.actions]]\nuses = "example/nope"'
  assert_bad_workspace "agentboard/run-cmd requires input cmd" $'[[sources]]\nid = "md"\n[sources.source]\nkind = "qmd"\ncollections = ["test"]\nquery = "ready"\n[[sources.actions]]\nuses = "agentboard/run-cmd"'
  assert_bad_workspace "agentboard/create-worktree requires input branch" $'[[sources]]\nid = "md"\n[sources.source]\nkind = "qmd"\ncollections = ["test"]\nquery = "ready"\n[[sources.actions]]\nuses = "agentboard/create-worktree"\n[sources.actions.with]\nrepo = "/tmp/repo"\nroot = "/tmp/root"'
}