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

@test "cancelled Action attempts retry on the next Run" {
  write_item AB-1
  write_workspace "if [ ! -e '$TMP/allow' ]; then echo started >> '$TMP/started'; trap '' INT; sleep 5; else echo retried >> '$TMP/retried'; fi"

  "$AB" --color never run "$TMP/workspace.toml" >"$TMP/run.out" 2>"$TMP/run.err" &
  RUN_PID=$!
  for _ in {1..100}; do
    [ -e "$TMP/started" ] && break
    sleep 0.05
  done
  [ -e "$TMP/started" ]

  kill -INT "$RUN_PID"
  wait_status=0
  wait "$RUN_PID" || wait_status=$?
  [ "$wait_status" -eq 130 ]

  /usr/bin/python3 - "$(actions_store_file)" <<'PY'
import json, pathlib, sys
records = [json.loads(line) for line in pathlib.Path(sys.argv[1]).read_text().splitlines()]
assert len(records) == 1
assert records[0]["outcome"] == "cancelled"
PY

  touch "$TMP/allow"
  run "$AB" --color never run "$TMP/workspace.toml"
  [ "$status" -eq 0 ]
  [ -e "$TMP/retried" ]

  /usr/bin/python3 - "$(actions_store_file)" <<'PY'
import json, pathlib, sys
records = [json.loads(line) for line in pathlib.Path(sys.argv[1]).read_text().splitlines()]
assert [record["outcome"] for record in records] == ["cancelled", "success"]
PY
}

@test "Ctrl-C kills run-cmd descendants, persists output, and exits 130" {
  write_item AB-1
  write_workspace "printf before; printf error-before >&2; echo started > '$TMP/started'; (sleep 2; echo descendant-finished > '$TMP/descendant') & wait"

  "$AB" --color never run "$TMP/workspace.toml" >"$TMP/run.out" 2>"$TMP/run.err" &
  RUN_PID=$!
  for _ in {1..100}; do
    [ -e "$TMP/started" ] && break
    sleep 0.05
  done
  [ -e "$TMP/started" ]

  kill -INT "$RUN_PID"
  wait_status=0
  wait "$RUN_PID" || wait_status=$?
  [ "$wait_status" -eq 130 ]
  [ ! -e "$TMP/descendant" ]

  /usr/bin/python3 - "$(actions_store_file)" <<'PY'
import json, pathlib, sys
record = json.loads(pathlib.Path(sys.argv[1]).read_text().splitlines()[0])
assert record["outcome"] == "cancelled"
assert record["stdout"] == "before"
assert record["stderr"] == "error-before"
assert record["message"] == "action cancelled"
PY
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
