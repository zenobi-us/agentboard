#!/usr/bin/env bats

load test_helper

setup() { setup_agentboard_test; }
teardown() { teardown_agentboard_test; }

@test "normal run executes actions with identity environment and writes valid Store records" {
  write_item AB-1 "Stored Item"
  write_workspace "printf '%s|%s|%s' \"\$AGENTBOARD_WORKSPACE_ID\" \"\$AGENTBOARD_SOURCE_ID\" \"\$AGENTBOARD_ITEM_ID\""

  run "$AB" --color never run "$TMP/workspace.toml"

  [ "$status" -eq 0 ]
  local items actions
  items="$(items_store_file)"
  actions="$(actions_store_file)"
  [ -f "$items" ]
  [ -f "$actions" ]
  /usr/bin/python3 - "$items" "$actions" "$QMD_ITEMS/AB-1.md" <<'PY'
import json, pathlib, sys
item_lines = pathlib.Path(sys.argv[1]).read_text().splitlines()
action_lines = pathlib.Path(sys.argv[2]).read_text().splitlines()
item_id = sys.argv[3]
assert len(item_lines) == 1
assert len(action_lines) == 1
item = json.loads(item_lines[0])
action = json.loads(action_lines[0])
assert item["id"] == item_id
assert item["reference_id"] == "AB-1"
assert item["title"] == "Stored Item"
assert action["item_id"] == item_id
assert action["source_id"] == "md"
assert action["uses"] == "agentboard/run-cmd"
assert action["success"] is True
assert action["rendered_action_hash"]
assert f"|md|{item_id}" in action["stdout"]
assert action["ts"]
PY
}

@test "Store appends item observations while retaining one successful action attempt" {
  write_item AB-1
  write_workspace "true"

  run "$AB" run "$TMP/workspace.toml"
  [ "$status" -eq 0 ]
  run "$AB" run "$TMP/workspace.toml"
  [ "$status" -eq 0 ]

  [ "$(wc -l < "$(items_store_file)")" -eq 2 ]
  [ "$(wc -l < "$(actions_store_file)")" -eq 1 ]
}

@test "duplicate QMD reference IDs keep distinct document identities" {
  write_item AB-1
  cp "$QMD_ITEMS/AB-1.md" "$QMD_ITEMS/duplicate.md"
  write_workspace "true"

  run "$AB" --color never run "$TMP/workspace.toml"

  [ "$status" -eq 0 ]
  [ "$(wc -l < "$(items_store_file)")" -eq 2 ]
  [ "$(wc -l < "$(actions_store_file)")" -eq 2 ]
  /usr/bin/python3 - "$(items_store_file)" <<'PY'
import json, pathlib, sys
items = [json.loads(line) for line in pathlib.Path(sys.argv[1]).read_text().splitlines()]
assert len({item["id"] for item in items}) == 2
assert {item["reference_id"] for item in items} == {"AB-1"}
PY
}