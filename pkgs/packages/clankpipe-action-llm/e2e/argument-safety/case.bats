#!/usr/bin/env bats

load '../helpers/test_helper.bash'

setup() { setup_case; }
teardown() { teardown_case; }

@test "passes shell-like reference IDs as one prompt argument" {
  run run_agentboard

  [ "$status" -eq 0 ]
  jq -e '.prompt == "/implement TASK-\u0027; touch injected"' "$FAKE_LLM_RECORD"
  [ ! -e "$TEST_TMPDIR/injected" ]
}
