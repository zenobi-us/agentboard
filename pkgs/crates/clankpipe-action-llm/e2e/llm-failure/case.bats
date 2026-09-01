#!/usr/bin/env bats

load '../helpers/test_helper.bash'

setup() {
  setup_case
  export FAKE_LLM_EXIT_CODE=7
}

teardown() { teardown_case; }

@test "reports a non-zero LLM exit code" {
  run run_agentboard

  [ "$status" -eq 1 ]
  jq -e '.sources[0].actions[0].result.message == "command exited with 7"' <<<"$output"
  jq -e '.prompt == "/implement TASK-1"' "$FAKE_LLM_RECORD"
}
