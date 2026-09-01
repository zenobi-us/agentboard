#!/usr/bin/env bats

load '../helpers/test_helper.bash'

setup() { setup_case; }
teardown() { teardown_case; }

@test "runs the LLM action in the worktree" {
  run run_agentboard

  [ "$status" -eq 0 ]
  jq -e '.sources[0].items | length == 1' <<<"$output"
  jq -e '.sources[0].actions[1].result.outcome == "success"' <<<"$output"
  jq -e '.prompt == "/implement TASK-1"' "$FAKE_LLM_RECORD"
  jq -e --arg cwd "$E2E_REPO/worktree" '.cwd == $cwd' "$FAKE_LLM_RECORD"
  assert_worktree_branch "agentboard/test"
}
