#!/usr/bin/env bats

setup() {
  MOCK_DIR="$(mktemp -d)"
  export PATH="${MOCK_DIR}:${PATH}"
  export MOCK_ARGS_FILE="${MOCK_DIR}/args"
}

teardown() {
  rm -rf "${MOCK_DIR}"
}

@test "setup defines the ClankPipe archive URL" {
  run grep -F "readonly SOURCE_ARCHIVE='https://github.com/zenobi-us/clankpipe/archive/refs/heads/main.tar.gz'" "${BATS_TEST_DIRNAME}/../../apps/demo/setup.sh"
  [ "$status" -eq 0 ]
}

@test "launcher accepts the ClankPipe environment controls" {
  run bash "${BATS_TEST_DIRNAME}/../../apps/demo/launch-agent.sh" --help
  [ "$status" -eq 0 ]
  [[ "$output" == *"CLANKPIPE_LAUNCHER"* ]]
  [[ "$output" == *"CLANKPIPE_LAUNCH_MODE"* ]]
}

@test "launcher honors CLANKPIPE_LAUNCHER" {
  cat >"${MOCK_DIR}/xterm" <<'EOF'
#!/usr/bin/env bash
printf '%s\n' "$*" >"${MOCK_ARGS_FILE}"
EOF
  chmod +x "${MOCK_DIR}/xterm"

  run env CLANKPIPE_LAUNCHER=xterm bash "${BATS_TEST_DIRNAME}/../../apps/demo/launch-agent.sh" launch implement 66
  [ "$status" -eq 0 ]
  [[ "$(cat "${MOCK_ARGS_FILE}")" == *"implement-66"* ]]
  [[ "$(cat "${MOCK_ARGS_FILE}")" == *"/implement 66"* ]]
}

@test "launcher honors CLANKPIPE_LAUNCH_MODE for Zellij" {
  cat >"${MOCK_DIR}/zellij" <<'EOF'
#!/usr/bin/env bash
printf '%s\n' "$*" >"${MOCK_ARGS_FILE}"
EOF
  chmod +x "${MOCK_DIR}/zellij"

  run env CLANKPIPE_LAUNCHER=zellij CLANKPIPE_LAUNCH_MODE=tab bash "${BATS_TEST_DIRNAME}/../../apps/demo/launch-agent.sh" launch review 66
  [ "$status" -eq 0 ]
  [ "$(cat "${MOCK_ARGS_FILE}")" = "action new-tab --cwd ${PWD} --name issue-66 -- pi /review 66" ]
}
