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
  local expected
  expected="$(printf '%s\tready\tfailed\tFailing Item\n%s\tready\tpending\tPending Item\n%s\tready\tsucceeded\tPassing Item' \
    "$QMD_ITEMS/failed/FAIL-1.md" \
    "$QMD_ITEMS/pending/PEND-1.md" \
    "$QMD_ITEMS/success/PASS-1.md")"
  [ "$output" = "$expected" ]

  run "$AB" list "$TMP/workspace.toml" --json
  [ "$status" -eq 0 ]
  printf '%s' "$output" | /usr/bin/python3 -c 'import json,sys; rows=json.load(sys.stdin); assert [row["item"]["reference_id"] for row in rows] == ["FAIL-1", "PEND-1", "PASS-1"]; assert len({row["item"]["id"] for row in rows}) == 3; assert {row["action_state"] for row in rows} == {"failed", "succeeded", "pending"}; assert all(row["source_slug"] for row in rows)'
}

@test "show returns plain and JSON item details with attempts" {
  write_item AB-1 "Shown Item"
  write_workspace "true"
  run "$AB" run "$TMP/workspace.toml"
  [ "$status" -eq 0 ]

  local item_id="$QMD_ITEMS/AB-1.md"
  run "$AB" show "$TMP/workspace.toml" "$item_id"
  [ "$status" -eq 0 ]
  [[ "$output" == *"$item_id"$'\nShown Item\nready'* ]]
  [[ "$output" == *"action#0 agentboard/run-cmd outcome=success"* ]]

  run "$AB" show "$TMP/workspace.toml" "$item_id" --json
  [ "$status" -eq 0 ]
  printf '%s' "$output" | /usr/bin/python3 -c 'import json,sys; value=json.load(sys.stdin); assert value["item"]["id"].endswith("/AB-1.md"); assert value["item"]["reference_id"] == "AB-1"; assert value["actions"][0]["outcome"] == "success"; assert "success" not in value["actions"][0]; assert value["source_slug"]'

  run "$AB" show "$TMP/workspace.toml" MISSING
  [ "$status" -ne 0 ]
  [[ "$output" == *"item MISSING not found"* ]]
}

@test "duplicate reference IDs remain distinct by QMD document identity" {
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

  "$AB" list "$TMP/workspace.toml" --json > "$TMP/list.json"
  local item_id
  item_id="$(/usr/bin/python3 - "$TMP/list.json" <<'PY'
import json, sys
rows = json.load(open(sys.argv[1]))
assert len(rows) == 2
assert {row["item"]["reference_id"] for row in rows} == {"SAME-1"}
assert len({row["item"]["id"] for row in rows}) == 2
print(next(row["item"]["id"] for row in rows if row["item"]["source_id"] == "one"))
PY
)"
  run "$AB" show "$TMP/workspace.toml" "$item_id"
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
  [ "$output" = "$QMD_ITEMS/AB-1.md"$'\tdoing\tpending\tNew Title' ]
  run "$AB" show "$TMP/workspace.toml" "$QMD_ITEMS/AB-1.md"
  [[ "$output" == *"$QMD_ITEMS/AB-1.md"$'\nNew Title\ndoing'* ]]
  [ "$(wc -l < "$(items_store_file)")" -eq 2 ]
}

@test "legacy item Store errors explain how to rebuild" {
  write_item AB-1
  write_workspace "true"
  run "$AB" run "$TMP/workspace.toml"
  [ "$status" -eq 0 ]

  local items
  items="$(items_store_file)"
  /usr/bin/python3 - "$items" <<'PY'
import json, pathlib, sys
path = pathlib.Path(sys.argv[1])
records = [json.loads(line) for line in path.read_text().splitlines()]
for record in records:
    record.pop("reference_id")
path.write_text("".join(json.dumps(record) + "\n" for record in records))
PY

  run "$AB" list "$TMP/workspace.toml"

  [ "$status" -ne 0 ]
  [[ "$output" == *"$items"* ]]
  [[ "$output" == *"line 1"* ]]
  [[ "$output" == *"reference_id"* ]]
  [[ "$output" == *"rebuild"* ]]
}