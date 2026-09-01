#!/usr/bin/env bats

load '../helpers/test_helper.bash'

setup() { setup_case; }
teardown() { teardown_case; }

@test "reports malformed QMD output" {
  run run_agentboard

  [ "$status" -eq 1 ]
  jq -e '.sources[0].error | contains("JSON Parse error")' <<<"$output"
  [ ! -e "$FAKE_LLM_RECORD" ]
}
