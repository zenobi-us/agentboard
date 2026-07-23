#!/usr/bin/env bats

load test_helper

setup() { setup_agentboard_test; }
teardown() { teardown_agentboard_test; }

@test "successful actions do not rerun" {
  write_item AB-1
  write_workspace "echo run >> '$TMP/ran'"

  run "$AB" run "$TMP/workspace.toml"
  [ "$status" -eq 0 ]
  run "$AB" run "$TMP/workspace.toml"
  [ "$status" -eq 0 ]

  [ "$(wc -l < "$TMP/ran")" -eq 1 ]
}

@test "failed actions retry until they succeed" {
  write_item AB-1
  write_workspace "if [ ! -f '$TMP/marker' ]; then touch '$TMP/marker'; exit 1; fi; echo ok >> '$TMP/ran'"

  run "$AB" run "$TMP/workspace.toml"
  [ "$status" -ne 0 ]
  run "$AB" run "$TMP/workspace.toml"
  [ "$status" -eq 0 ]

  [ "$(cat "$TMP/ran")" = "ok" ]
  [ "$(wc -l < "$(actions_store_file)")" -eq 2 ]
}

@test "items are sorted and a failed action stops only that item chain" {
  write_item AB-2
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
cmd = '''echo "first-{{ item.reference_id }}" >> "$TMP/order"; if [ "{{ item.reference_id }}" = "AB-1" ]; then exit 1; fi'''

[[sources.actions]]
uses = "agentboard/run-cmd"
[sources.actions.with]
cmd = '''echo "second-{{ item.reference_id }}" >> "$TMP/order"'''
EOF

  run "$AB" --color never run "$TMP/workspace.toml"

  [ "$status" -ne 0 ]
  [ "$(cat "$TMP/order")" = $'first-AB-1\nfirst-AB-2\nsecond-AB-2' ]
}

@test "a failed Source does not prevent sibling Sources completing" {
  write_collection_item good AB-2
  cat > "$TMP/workspace.toml" <<EOF
[[sources]]
id = "fails"
[sources.source]
kind = "qmd"
collections = ["fail"]
query = "ready"

[[sources]]
id = "works"
[sources.source]
kind = "qmd"
collections = ["good"]
query = "ready"

[[sources.actions]]
uses = "agentboard/run-cmd"
[sources.actions.with]
cmd = '''echo "{{ item.reference_id }}" >> "$TMP/completed"'''
EOF
  export QMD_FAIL_COLLECTION=fail

  run "$AB" --color never run "$TMP/workspace.toml"

  [ "$status" -ne 0 ]
  [[ "$output" == *"source fails failed"* ]]
  [ "$(cat "$TMP/completed")" = "AB-2" ]
}

@test "changed rendered inputs create new retry identity" {
  write_item AB-1 "First Title"
  write_workspace "echo '{{ item.title }}' >> '$TMP/ran'"

  run "$AB" run "$TMP/workspace.toml"
  [ "$status" -eq 0 ]
  write_item AB-1 "Second Title"
  run "$AB" run "$TMP/workspace.toml"
  [ "$status" -eq 0 ]

  [ "$(cat "$TMP/ran")" = $'First Title\nSecond Title' ]
  [ "$(wc -l < "$(actions_store_file)")" -eq 2 ]
}

@test "changed source configuration creates a new action Store" {
  write_item AB-1
  write_workspace "echo run >> '$TMP/ran'"

  run "$AB" run "$TMP/workspace.toml"
  [ "$status" -eq 0 ]
  sed -i 's/query = "status:ready"/query = "status:changed"/' "$TMP/workspace.toml"
  run "$AB" run "$TMP/workspace.toml"
  [ "$status" -eq 0 ]

  [ "$(wc -l < "$TMP/ran")" -eq 2 ]
  [ "$(find "$XDG_DATA_HOME/agentboard" -name 'actions-*.jsonl' | wc -l)" -eq 2 ]
}