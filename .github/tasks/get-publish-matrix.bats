#!/usr/bin/env bats

setup() {
  SCRIPT="${BATS_TEST_DIRNAME}/get-publish-matrix"
  MOCK_DIR="$(mktemp -d)"
  export PATH="${MOCK_DIR}:${PATH}"
  export MOON_MOCK_OUTPUT='{"projects":[]}'
  export MOON_ARGS_FILE="${MOCK_DIR}/args"

  cat >"${MOCK_DIR}/moon" <<'EOF'
#!/usr/bin/env bash
printf '%s\n' "$*" >"${MOON_ARGS_FILE}"
printf '%s\n' "${MOON_MOCK_OUTPUT}"
EOF
  chmod +x "${MOCK_DIR}/moon"
}

teardown() {
  rm -rf "${MOCK_DIR}"
}

@test "requires base and head" {
  run bash "${SCRIPT}"
  [ "$status" -ne 0 ]
  [[ "$output" == *"Missing base argument"* ]]

  run bash "${SCRIPT}" base
  [ "$status" -ne 0 ]
  [[ "$output" == *"Missing head argument"* ]]
}

@test "returns sorted target-only entries for affected publishable projects" {
  export MOON_MOCK_OUTPUT='{
    "projects": [
      {"id":"docs","tasks":{"publish":{}}},
      {"id":"private","tasks":{"test":{}}},
      {"id":"agentboard","tasks":{"publish":{}}},
      {"id":"docs","tasks":{"publish":{}}}
    ]
  }'

  run bash "${SCRIPT}" abc123 def456
  [ "$status" -eq 0 ]
  run jq -e '. == [{"target":"agentboard"},{"target":"docs"}]' <<<"$output"
  [ "$status" -eq 0 ]
  [ "$(cat "${MOON_ARGS_FILE}")" = "query projects --affected" ]
}

@test "exports Moon affected range" {
  export MOON_MOCK_OUTPUT='{"projects":[]}'
  cat >"${MOCK_DIR}/moon" <<'EOF'
#!/usr/bin/env bash
[[ "${MOON_BASE}" == "abc123" ]]
[[ "${MOON_HEAD}" == "def456" ]]
printf '%s\n' '{"projects":[]}'
EOF
  chmod +x "${MOCK_DIR}/moon"

  run bash "${SCRIPT}" abc123 def456
  [ "$status" -eq 0 ]
  [ "$output" = "[]" ]
}
