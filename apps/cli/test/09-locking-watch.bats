#!/usr/bin/env bats

load test_helper

setup() { setup_agentboard_test; }
teardown() { teardown_agentboard_test; }

@test "watch holds the workspace lock while dry-run remains available" {
  write_item AB-1
  write_workspace "true"

  "$AB" --color never --log-file "$TMP/watch.jsonl" watch "$TMP/workspace.toml" --interval 60s >"$TMP/watch.out" 2>"$TMP/watch.err" &
  WATCH_PID=$!
  wait_for_pattern "$TMP/watch.jsonl" '"stage":"watch.cycle.complete"'

  run "$AB" --color never run "$TMP/workspace.toml"
  [ "$status" -ne 0 ]
  [[ "$output" == *"workspace lock is held"* ]]

  run "$AB" run "$TMP/workspace.toml" --dry-run
  [ "$status" -eq 0 ]

  kill -INT "$WATCH_PID"
  wait "$WATCH_PID"
  [ "$?" -eq 0 ]
  WATCH_PID=""
  grep -q 'watch .* stopped' "$TMP/watch.err"
  grep -q '"stage":"watch.stop"' "$TMP/watch.jsonl"
}

@test "watch reports failure then retries successfully on a later cycle" {
  write_item AB-1
  write_workspace "if [ ! -f '$TMP/allow' ]; then exit 1; fi; echo ok >> '$TMP/ran'"

  "$AB" --color never --log-file "$TMP/watch.jsonl" watch "$TMP/workspace.toml" --interval 1s >"$TMP/watch.out" 2>"$TMP/watch.err" &
  WATCH_PID=$!
  wait_for_pattern "$TMP/watch.jsonl" '"stage":"watch.cycle.failed"'
  touch "$TMP/allow"
  wait_for_pattern "$TMP/watch.jsonl" '"stage":"watch.cycle.complete"' 160

  kill -INT "$WATCH_PID"
  wait "$WATCH_PID"
  [ "$?" -eq 0 ]
  WATCH_PID=""

  [ "$(cat "$TMP/ran")" = "ok" ]
  grep -q '"outcome":"fail"' "$TMP/watch.jsonl"
  grep -q '"outcome":"pass"' "$TMP/watch.jsonl"
}