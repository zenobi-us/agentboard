#!/usr/bin/env bats

load test_helper

setup() { setup_agentboard_test; }
teardown() { teardown_agentboard_test; }

@test "normal output uses stderr and diagnostic JSONL excludes command output" {
  write_item AB-1
  write_workspace "echo action-output"

  "$AB" --color never --log-file "$TMP/events.jsonl" run "$TMP/workspace.toml" >"$TMP/stdout" 2>"$TMP/stderr"

  [ ! -s "$TMP/stdout" ]
  grep -q "run .* starting" "$TMP/stderr"
  grep -q "source md complete: 1 items" "$TMP/stderr"
  grep -q '"stage":"run.complete"' "$TMP/events.jsonl"
  ! grep -q 'action-output' "$TMP/events.jsonl"
  ! grep -q '"stdout"' "$TMP/events.jsonl"
  ! grep -q $'\033' "$TMP/stderr"
  /usr/bin/python3 - "$TMP/events.jsonl" <<'PY'
import json, pathlib, sys
for line in pathlib.Path(sys.argv[1]).read_text().splitlines():
    event = json.loads(line)
    assert event["ts"]
    assert event["invocation"]
    assert event["level"]
    assert event["stage"]
PY
}

@test "verbose shows successful action stdout and stderr" {
  write_item AB-1
  write_workspace "echo visible-out; echo visible-err >&2"

  run "$AB" -v --color never run "$TMP/workspace.toml"

  [ "$status" -eq 0 ]
  [[ "$output" == *"visible-out"* ]]
  [[ "$output" == *"visible-err"* ]]
}

@test "quiet suppresses success output but never suppresses failures" {
  write_item AB-1
  write_workspace "true"

  "$AB" -q run "$TMP/workspace.toml" >"$TMP/stdout" 2>"$TMP/stderr"
  [ ! -s "$TMP/stdout" ]
  [ ! -s "$TMP/stderr" ]

  rm -rf "$XDG_DATA_HOME"
  write_workspace "echo failed-out; echo failed-err >&2; exit 1"
  run "$AB" -q --color never run "$TMP/workspace.toml"
  [ "$status" -ne 0 ]
  [[ "$output" == *"failed-out"* ]]
  [[ "$output" == *"failed-err"* ]]
}

@test "color controls honor never always and NO_COLOR" {
  write_item AB-1
  write_workspace "true"

  "$AB" --color never run "$TMP/workspace.toml" 2>"$TMP/never"
  ! grep -q $'\033' "$TMP/never"

  rm -rf "$XDG_DATA_HOME"
  "$AB" --color always run "$TMP/workspace.toml" 2>"$TMP/always"
  grep -q $'\033' "$TMP/always"

  rm -rf "$XDG_DATA_HOME"
  NO_COLOR=1 "$AB" --color auto run "$TMP/workspace.toml" 2>"$TMP/no-color"
  ! grep -q $'\033' "$TMP/no-color"
}