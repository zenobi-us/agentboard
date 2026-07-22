#!/usr/bin/env bats

load test_helper

setup() { setup_agentboard_test; }
teardown() { teardown_agentboard_test; }

# Keeps failure assertions consistent while loader errors gain Registry context.
assert_bad_workspace() {
  local expected="$1"
  local body="$2"
  printf '%s\n' "$body" > "$TMP/bad.toml"
  run "$AB" run "$TMP/bad.toml"
  [ "$status" -ne 0 ]
  [[ "$output" == *"$expected"* ]]
}

# Uses a real Draft 7 validator so schema shape cannot pass through string checks alone.
@test "schema composes every registered variant and rejects invalid config" {
  run "$AB" schema

  [ "$status" -eq 0 ]
  printf '%s' "$output" > "$TMP/schema.json"
  /usr/bin/python3 - "$TMP/schema.json" <<'PY'
import json, sys
value = json.load(open(sys.argv[1]))
source = value["properties"]["sources"]["items"]["properties"]
source_ids = [variant["properties"]["kind"]["enum"][0] for variant in source["source"]["oneOf"]]
action_ids = [variant["properties"]["uses"]["enum"][0] for variant in source["actions"]["items"]["oneOf"]]
assert source_ids == ["github", "jira", "qmd"]
assert action_ids == ["agentboard/create-worktree", "agentboard/run-cmd"]
assert all(name.startswith("source::") or name.startswith("action::") for name in value["definitions"])
PY

  printf '%s' '{"sources":[{"id":"md","source":{"kind":"qmd","collections":["tasks"],"query":"ready"},"actions":[{"uses":"agentboard/run-cmd","with":{"cmd":"true"}}]}]}' > "$TMP/valid.json"
  run /usr/bin/jsonschema -i "$TMP/valid.json" "$TMP/schema.json"
  [ "$status" -eq 0 ]

  local invalid
  for invalid in \
    '{"sources":[{"id":"md","source":{"kind":"unknown"}}]}' \
    '{"sources":[{"id":"md","source":{"kind":"qmd","collections":["tasks"]}}]}' \
    '{"sources":[{"id":"md","source":{"kind":"qmd","collections":["tasks"],"query":"ready","extra":true}}]}' \
    '{"sources":[{"id":"md","source":{"kind":"qmd","collections":["tasks"],"query":"ready"},"actions":[{"uses":"unknown","with":{}}]}]}'
  do
    printf '%s' "$invalid" > "$TMP/invalid.json"
    run /usr/bin/jsonschema -i "$TMP/invalid.json" "$TMP/schema.json"
    [ "$status" -ne 0 ]
  done
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

# Config must fail before even diagnostic logging creates a user-visible file.
@test "invalid Workspace config fails before output side effects" {
  printf '%s\n' 'sources = []' 'unknown = true' > "$TMP/bad.toml"

  run "$AB" --log-file "$TMP/events.jsonl" run "$TMP/bad.toml"

  [ "$status" -ne 0 ]
  [ ! -e "$TMP/events.jsonl" ]
}

@test "source-specific invariants reject invalid QMD Jira and GitHub configs" {
  assert_bad_workspace "requires at least one collection" $'[[sources]]\nid = "md"\n[sources.source]\nkind = "qmd"\ncollections = []\nquery = "ready"'
  assert_bad_workspace "requires query" $'[[sources]]\nid = "md"\n[sources.source]\nkind = "qmd"\ncollections = ["test"]\nquery = ""'
  assert_bad_workspace "limit must be greater than zero" $'[[sources]]\nid = "md"\n[sources.source]\nkind = "qmd"\ncollections = ["test"]\nquery = "ready"\nlimit = 0'
  assert_bad_workspace "requires site" $'[[sources]]\nid = "jira"\n[sources.source]\nkind = "jira"\nsite = ""\njql = "project = AB"'
  assert_bad_workspace "requires status_map" $'[[sources]]\nid = "github"\n[sources.source]\nkind = "github"\nmode = "issue"\nquery = "repo:owner/repo"\n[sources.source.credentials]\nhelper = "printf token"\n[sources.source.status_map]'
}

@test "action validation rejects unknown actions and missing required inputs" {
  assert_bad_workspace "unknown action registration example/nope" $'[[sources]]\nid = "md"\n[sources.source]\nkind = "qmd"\ncollections = ["test"]\nquery = "ready"\n[[sources.actions]]\nuses = "example/nope"'
  assert_bad_workspace 'missing field `cmd`' $'[[sources]]\nid = "md"\n[sources.source]\nkind = "qmd"\ncollections = ["test"]\nquery = "ready"\n[[sources.actions]]\nuses = "agentboard/run-cmd"'
  assert_bad_workspace 'missing field `branch`' $'[[sources]]\nid = "md"\n[sources.source]\nkind = "qmd"\ncollections = ["test"]\nquery = "ready"\n[[sources.actions]]\nuses = "agentboard/create-worktree"\n[sources.actions.with]\nrepo = "/tmp/repo"\nroot = "/tmp/root"'
}