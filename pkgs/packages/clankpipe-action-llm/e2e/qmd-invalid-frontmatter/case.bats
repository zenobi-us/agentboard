#!/usr/bin/env bats

load '../helpers/test_helper.bash'

setup() { setup_case; }
teardown() { teardown_case; }

@test "fails before the LLM action when QMD frontmatter is invalid" {
  run run_agentboard

  [ "$status" -eq 1 ]
  jq -e '.sources[0].error | contains("qmd mapping status")' <<<"$output"
  [ ! -e "$FAKE_LLM_RECORD" ]
}
