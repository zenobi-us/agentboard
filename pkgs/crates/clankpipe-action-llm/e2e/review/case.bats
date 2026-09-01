#!/usr/bin/env bats

load '../helpers/test_helper.bash'

setup() { setup_case; }
teardown() { teardown_case; }

@test "passes the review prompt to the LLM action" {
  run run_agentboard

  [ "$status" -eq 0 ]
  jq -e '.sources[0].actions[1].result.outcome == "success"' <<<"$output"
  jq -e '.prompt == "/review TASK-1"' "$FAKE_LLM_RECORD"
  jq -e --arg cwd "$E2E_REPO/worktree" '.cwd == $cwd' "$FAKE_LLM_RECORD"
}
