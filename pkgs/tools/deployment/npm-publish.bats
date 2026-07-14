#!/usr/bin/env bats

setup() {
  SCRIPT="${BATS_TEST_DIRNAME}/npm-publish"
  REPO="$(mktemp -d)"
  MOCK_DIR="$(mktemp -d)"
  export PATH="${MOCK_DIR}:${PATH}"
  export NPM_ARGS_FILE="${REPO}/npm-args"
  printf '%s\n' '{"name":"example","version":"1.2.3"}' >"${REPO}/package.json"
  cat >"${MOCK_DIR}/npm" <<'EOF'
#!/usr/bin/env bash
printf '%s\n' "$*" >"${NPM_ARGS_FILE}"
EOF
  chmod +x "${MOCK_DIR}/npm"
  cd "${REPO}"
}

teardown() {
  rm -rf "${REPO}" "${MOCK_DIR}"
}

@test "latest publishes source version on latest channel" {
  run env PUBLISH_CHANNEL=latest PUBLISH_VERSION=1.2.3 bash "${SCRIPT}"
  [ "$status" -eq 0 ]
  [ "$(jq -r .version package.json)" = "1.2.3" ]
  [ "$(cat "${NPM_ARGS_FILE}")" = "publish --access public --tag latest" ]
}

@test "next applies resolver version and npm channel" {
  run env PUBLISH_CHANNEL=next PUBLISH_VERSION=1.3.0-next.7.2 bash "${SCRIPT}"
  [ "$status" -eq 0 ]
  [ "$(jq -r .version package.json)" = "1.3.0-next.7.2" ]
  [ "$(cat "${NPM_ARGS_FILE}")" = "publish --access public --tag next" ]
}

@test "old PUBLISH_TAG does not satisfy channel contract" {
  run env PUBLISH_TAG=next PUBLISH_VERSION=1.3.0-next.7.1 bash "${SCRIPT}"
  [ "$status" -ne 0 ]
  [[ "$output" == *"PUBLISH_CHANNEL is required"* ]]
}
