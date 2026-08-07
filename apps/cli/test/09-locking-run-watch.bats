#!/usr/bin/env bats

load test_helper

setup() { setup_agentboard_test; }
teardown() { teardown_agentboard_test; }

@test "run watch holds the workspace lock while dry-run remains available" {
  write_item AB-1
  write_workspace "true"

  "$AB" --color never --log-file "$TMP/run.jsonl" run "$TMP/workspace.toml" --watch --interval 60s >"$TMP/run.out" 2>"$TMP/run.err" &
  WATCH_PID=$!
  wait_for_pattern "$TMP/run.jsonl" '"stage":"run.watch.cycle.complete"'

  run "$AB" --color never run "$TMP/workspace.toml"
  [ "$status" -ne 0 ]
  [[ "$output" == *"workspace lock is held"* ]]

  run "$AB" run "$TMP/workspace.toml" --dry-run
  [ "$status" -eq 0 ]

  kill -INT "$WATCH_PID"
  wait_status=0
  wait "$WATCH_PID" || wait_status=$?
  [ "$wait_status" -eq 130 ]
  WATCH_PID=""
  grep -q 'run .* stopped' "$TMP/run.err"
  [ "$(grep -c 'next Run in 60s' "$TMP/run.err")" -eq 1 ]
  ! grep -q $'\r' "$TMP/run.err"
  [ "$(grep -c '"stage":"run.watch.wait"' "$TMP/run.jsonl")" -eq 1 ]
  grep -q '"stage":"run.watch.stop"' "$TMP/run.jsonl"
}

@test "watched dry-run does not hold the workspace lock" {
  write_item AB-1
  write_workspace "echo ran >> '$TMP/ran'"

  "$AB" --color never --log-file "$TMP/dry-run.jsonl" run "$TMP/workspace.toml" --watch --dry-run --interval 60s >"$TMP/dry-run.out" 2>"$TMP/dry-run.err" &
  WATCH_PID=$!
  wait_for_pattern "$TMP/dry-run.jsonl" '"stage":"run.watch.cycle.complete"'
  [ ! -e "$XDG_DATA_HOME/agentboard" ]

  run "$AB" --color never run "$TMP/workspace.toml"
  [ "$status" -eq 0 ]
  [ "$(cat "$TMP/ran")" = "ran" ]

  kill -INT "$WATCH_PID"
  wait_status=0
  wait "$WATCH_PID" || wait_status=$?
  [ "$wait_status" -eq 130 ]
  WATCH_PID=""
}

@test "run watch reports failure then retries successfully on a later cycle" {
  write_item AB-1
  write_workspace "if [ ! -f '$TMP/allow' ]; then exit 1; fi; echo ok >> '$TMP/ran'"

  "$AB" --color never --log-file "$TMP/run.jsonl" run "$TMP/workspace.toml" --watch --interval 1s >"$TMP/run.out" 2>"$TMP/run.err" &
  WATCH_PID=$!
  wait_for_pattern "$TMP/run.jsonl" '"stage":"run.watch.cycle.failed"'
  touch "$TMP/allow"
  wait_for_pattern "$TMP/run.jsonl" '"stage":"run.watch.cycle.complete"' 160

  kill -INT "$WATCH_PID"
  wait_status=0
  wait "$WATCH_PID" || wait_status=$?
  [ "$wait_status" -eq 130 ]
  WATCH_PID=""

  [ "$(cat "$TMP/ran")" = "ok" ]
  grep -q '"outcome":"fail"' "$TMP/run.jsonl"
  grep -q '"outcome":"pass"' "$TMP/run.jsonl"
}
