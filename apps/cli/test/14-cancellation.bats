#!/usr/bin/env bats

load test_helper

setup() { setup_agentboard_test; }
teardown() { teardown_agentboard_test; }

@test "first Ctrl-C cancels between work units and exits 130" {
  write_item AB-1
  write_workspace "echo ran >> '$TMP/ran'"
  export QMD_QUERY_SLEEP=2

  "$AB" --color never --log-file "$TMP/run.jsonl" run "$TMP/workspace.toml" >"$TMP/run.out" 2>"$TMP/run.err" &
  RUN_PID=$!
  wait_for_pattern "$TMP/run.jsonl" '"stage":"run.start"'

  kill -INT "$RUN_PID"
  wait_status=0
  wait "$RUN_PID" || wait_status=$?
  [ "$wait_status" -eq 130 ]
  [ ! -e "$TMP/ran" ]
  grep -q '"stage":"run.cancelled"' "$TMP/run.jsonl"
  ! grep -q '"stage":"action.succeeded"' "$TMP/run.jsonl"
}

@test "second Ctrl-C force-exits while running work cleans up" {
  write_item AB-1
  write_workspace "trap '' INT; echo started >> '$TMP/started'; sleep 5"

  "$AB" --color never run "$TMP/workspace.toml" >"$TMP/run.out" 2>"$TMP/run.err" &
  RUN_PID=$!
  for _ in {1..100}; do
    [ -e "$TMP/started" ] && break
    sleep 0.05
  done
  [ -e "$TMP/started" ]

  kill -INT "$RUN_PID"
  kill -INT "$RUN_PID"
  wait_status=0
  wait "$RUN_PID" || wait_status=$?
  [ "$wait_status" -eq 130 ]
}
