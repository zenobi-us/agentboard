#!/usr/bin/env bats

load test_helper

setup() { setup_agentboard_test; }
teardown() { teardown_agentboard_test; }

@test "dry-run renders pending actions without executing or writing Store files" {
  write_item AB-1 "Dry Run Item"
  write_workspace "echo bad > '$TMP/ran'"

  "$AB" --color never run "$TMP/workspace.toml" --dry-run >"$TMP/stdout" 2>"$TMP/stderr"

  grep -q "md $QMD_ITEMS/AB-1.md action#0 agentboard/run-cmd" "$TMP/stdout"
  grep -q '"cmd":"echo bad' "$TMP/stdout"
  grep -q 'run .* complete' "$TMP/stderr"
  [ ! -e "$TMP/ran" ]
  [ -z "$(store_files)" ]
}

@test "dry-run succeeds even when the rendered command would fail" {
  write_item AB-1
  write_workspace "exit 99"

  run "$AB" run "$TMP/workspace.toml" --dry-run

  [ "$status" -eq 0 ]
  [[ "$output" == *"exit 99"* ]]
}