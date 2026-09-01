#!/usr/bin/env bash

setup_case() {
  CASE_DIR="$BATS_TEST_DIRNAME"
  PACKAGE_DIR="$(cd "$CASE_DIR/../.." && pwd)"
  REPO_ROOT="$(cd "$PACKAGE_DIR/../../.." && pwd)"
  TEST_TMPDIR="$(mktemp -d)"
  export CASE_DIR PACKAGE_DIR REPO_ROOT TEST_TMPDIR
  export HOME="$TEST_TMPDIR/home"
  export E2E_REPO="$TEST_TMPDIR/repo"
  export FAKE_QMD_OUTPUT="$CASE_DIR/fixtures/qmd.json"
  export FAKE_LLM_RECORD="$TEST_TMPDIR/fake-llm.json"
  export PATH="$TEST_TMPDIR/bin:$PACKAGE_DIR/e2e/helpers:$PATH"

  mkdir -p "$HOME/.local/share/agentboard/plugins/npm/@agentboard" "$TEST_TMPDIR/bin" "$E2E_REPO"
  ln -s "$PACKAGE_DIR/e2e/helpers/fake-qmd" "$TEST_TMPDIR/bin/qmd"
  ln -s "$PACKAGE_DIR/e2e/helpers/fake-llm" "$TEST_TMPDIR/bin/fake-llm"
  ln -s "$PACKAGE_DIR" "$HOME/.local/share/agentboard/plugins/npm/@clankpipe/action-llm"

  git -C "$E2E_REPO" init -q -b main
  printf 'initial\n' > "$E2E_REPO/README.md"
  git -C "$E2E_REPO" add README.md
  git -C "$E2E_REPO" -c user.name=AgentBoard -c user.email=agentboard@example.test commit -qm initial
}

teardown_case() {
  rm -rf "$TEST_TMPDIR"
}

run_agentboard() {
  bun "$REPO_ROOT/apps/cli/src/cli/index.ts" run "$CASE_DIR/.agentboard.toml" --output-format json
}

assert_worktree_branch() {
  [ "$(git -C "$E2E_REPO/worktree" branch --show-current)" = "$1" ]
}
