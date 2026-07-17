#!/usr/bin/env bats

load test_helper

setup() { setup_agentboard_test; }
teardown() { teardown_agentboard_test; }

@test "list reports pending succeeded and failed items in ID order" {
  write_collection_item pending PEND-1 "Pending Item"
  write_collection_item success PASS-1 "Passing Item"
  write_collection_item failed FAIL-1 "Failing Item"
  cat > "$TMP/workspace.toml" <<'EOF'
[[sources]]
id = "pending"
[sources.source]
kind = "qmd"
collections = ["pending"]
query = "ready"

[[sources]]
id = "success"
[sources.source]
kind = "qmd"
collections = ["success"]
query = "ready"
[[sources.actions]]
uses = "agentboard/run-cmd"
[sources.actions.with]
cmd = "true"

[[sources]]
id = "failed"
[sources.source]
kind = "qmd"
collections = ["failed"]
query = "ready"
[[sources.actions]]
uses = "agentboard/run-cmd"
[sources.actions.with]
cmd = "exit 1"
EOF

  run "$AB" --color never run "$TMP/workspace.toml"
  [ "$status" -ne 0 ]

  run "$AB" list "$TMP/workspace.toml"
  [ "$status" -eq 0 ]
  [ "$output" = $'FAIL-1\tready\tfailed\tFailing Item\nPASS-1\tready\tsucceeded\tPassing Item\nPEND-1\tready\tpending\tPending Item' ]

  run "$AB" list "$TMP/workspace.toml" --json
  [ "$status" -eq 0 ]
  printf '%s' "$output" | /usr/bin/python3 -c 'import json,sys; rows=json.load(sys.stdin); assert [row["item"]["id"] for row in rows] == ["FAIL-1", "PASS-1", "PEND-1"]; assert {row["action_state"] for row in rows} == {"failed", "succeeded", "pending"}; assert all(row["source_slug"] for row in rows)'
}

@test "show returns plain and JSON item details with attempts" {
  write_item AB-1 "Shown Item"
  write_workspace "true"
  run "$AB" run "$TMP/workspace.toml"
  [ "$status" -eq 0 ]

  run "$AB" show "$TMP/workspace.toml" AB-1
  [ "$status" -eq 0 ]
  [[ "$output" == *$'AB-1\nShown Item\nready'* ]]
  [[ "$output" == *"action#0 agentboard/run-cmd success=true"* ]]

  run "$AB" show "$TMP/workspace.toml" AB-1 --json
  [ "$status" -eq 0 ]
  printf '%s' "$output" | /usr/bin/python3 -c 'import json,sys; value=json.load(sys.stdin); assert value["item"]["id"] == "AB-1"; assert value["actions"][0]["success"] is True; assert value["source_slug"]'

  run "$AB" show "$TMP/workspace.toml" MISSING
  [ "$status" -ne 0 ]
  [[ "$output" == *"item MISSING not found"* ]]
}

@test "qualified item references resolve ambiguity across source buckets" {
  write_collection_item one SAME-1 "First Bucket"
  write_collection_item two SAME-1 "Second Bucket"
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

  run "$AB" run "$TMP/workspace.toml"
  [ "$status" -eq 0 ]
  run "$AB" show "$TMP/workspace.toml" SAME-1
  [ "$status" -ne 0 ]
  [[ "$output" == *"ambiguous across Store item buckets"* ]]

  "$AB" list "$TMP/workspace.toml" --json > "$TMP/list.json"
  local slug
  slug="$(/usr/bin/python3 - "$TMP/list.json" <<'PY'
import json, sys
rows = json.load(open(sys.argv[1]))
print(next(row["source_slug"] for row in rows if row["item"]["source_id"] == "one"))
PY
)"
  run "$AB" show "$TMP/workspace.toml" "$slug:SAME-1"
  [ "$status" -eq 0 ]
  [[ "$output" == *"First Bucket"* ]]
}

@test "list and show use the latest append-only item observation" {
  write_item AB-1 "Old Title" ready
  cat > "$TMP/workspace.toml" <<'EOF'
[[sources]]
id = "md"
[sources.source]
kind = "qmd"
collections = ["test"]
query = "ready"
EOF

  run "$AB" run "$TMP/workspace.toml"
  [ "$status" -eq 0 ]
  write_item AB-1 "New Title" doing
  run "$AB" run "$TMP/workspace.toml"
  [ "$status" -eq 0 ]

  run "$AB" list "$TMP/workspace.toml"
  [ "$output" = $'AB-1\tdoing\tpending\tNew Title' ]
  run "$AB" show "$TMP/workspace.toml" AB-1
  [[ "$output" == *$'AB-1\nNew Title\ndoing'* ]]
  [ "$(wc -l < "$(items_store_file)")" -eq 2 ]
}