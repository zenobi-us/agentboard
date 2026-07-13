#!/usr/bin/env bats

setup() {
  SCRIPT="${BATS_TEST_DIRNAME}/get-publish-matrix"
  REPO_ROOT="${BATS_TEST_DIRNAME}/../.."
  BASE="abc123"
  HEAD="def456"
  RUN_NUMBER="481"

  MOCK_DIR="$(mktemp -d)"
  export PATH="${MOCK_DIR}:${PATH}"
  export MOON_MOCK_OUTPUT='{"projects":[]}'

  cat >"${MOCK_DIR}/moon" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail

if [[ "${1:-}" = "query" && "${2:-}" = "projects" ]]; then
  printf '%s\n' "${MOON_MOCK_OUTPUT}"
  exit 0
fi

echo "unexpected moon args: $*" >&2
exit 1
EOF
  chmod +x "${MOCK_DIR}/moon"
  cd "${REPO_ROOT}"
}

teardown() {
  rm -rf "${MOCK_DIR}"
}

@test "fails on missing payload" {
  run bash "${SCRIPT}"
  [ "$status" -eq 1 ]
  [[ "$output" == *"Missing JSON payload argument"* ]]
}

@test "prerelease matrix uses next minor version and skips non-publishable projects" {
  export MOON_MOCK_OUTPUT='{
    "projects": [
      {"id":"agentboard","source":"apps/cli","tasks":{"publish":{}}},
      {"id":"agentboard-core","source":"pkgs/crates/agentboard-core","tasks":{"test":{}}}
    ]
  }'

  run bash "${SCRIPT}" '{"releases_created":false}' "${BASE}" "${HEAD}" changed "" "${RUN_NUMBER}" normal
  [ "$status" -eq 0 ]
  run jq -e '. == {
    "publish":[{"project_id":"agentboard","version":"0.1.0-next.481","publish_tag":"next","release_tag":"0.1.0-next.481","source_sha":"def456"}],
    "skipped":[{"project_id":"agentboard-core","reason":"project has no publish task"}]
  }' <<<"$output"
  [ "$status" -eq 0 ]
}

@test "stable matrix uses manifest version" {
  export MOON_MOCK_OUTPUT='{
    "projects": [
      {"id":"agentboard","source":"apps/cli","tasks":{"publish":{}}}
    ]
  }'

  run bash "${SCRIPT}" '{"releases_created":true,"apps/cli--tag_name":"agentboard-v0.0.1"}' "${BASE}" "${HEAD}" changed "" "${RUN_NUMBER}" normal
  [ "$status" -eq 0 ]
  run jq -e '.publish[0].version == "0.0.1" and .publish[0].publish_tag == "latest" and .publish[0].release_tag == "agentboard-v0.0.1"' <<<"$output"
  [ "$status" -eq 0 ]
}

@test "hotfix prerelease bumps patch" {
  export MOON_MOCK_OUTPUT='{
    "projects": [
      {"id":"agentboard","source":"apps/cli","tasks":{"publish":{}}}
    ]
  }'

  run bash "${SCRIPT}" '{"releases_created":false}' "${BASE}" "${HEAD}" changed "" "${RUN_NUMBER}" hotfix
  [ "$status" -eq 0 ]
  run jq -e '.publish[0].version == "0.0.2-next.481"' <<<"$output"
  [ "$status" -eq 0 ]
}

@test "projects mode skips unknown and non-publishable IDs without failing" {
  export MOON_MOCK_OUTPUT='{
    "projects": [
      {"id":"agentboard","source":"apps/cli","tasks":{"publish":{}}},
      {"id":"agentboard-core","source":"pkgs/crates/agentboard-core","tasks":{"test":{}}}
    ]
  }'

  run bash "${SCRIPT}" '{"releases_created":false}' "${BASE}" "${HEAD}" projects 'missing, agentboard-core, agentboard' "${RUN_NUMBER}" normal
  [ "$status" -eq 0 ]
  run jq -e '
    (.publish | map(.project_id)) == ["agentboard"] and
    .skipped == [
      {"project_id":"agentboard-core","reason":"project has no publish task"},
      {"project_id":"missing","reason":"unknown Moon project"}
    ]
  ' <<<"$output"
  [ "$status" -eq 0 ]
}

@test "accepts payload from file" {
  export MOON_MOCK_OUTPUT='{"projects":[{"id":"agentboard","source":"apps/cli","tasks":{"publish":{}}}]}'
  payload_file="$(mktemp)"
  printf '%s' '{"releases_created":false}' >"${payload_file}"

  run bash "${SCRIPT}" "@${payload_file}" "${BASE}" "${HEAD}" changed "" "${RUN_NUMBER}" normal
  rm -f "${payload_file}"

  [ "$status" -eq 0 ]
  run jq -e '.publish | length == 1' <<<"$output"
  [ "$status" -eq 0 ]
}
