#!/usr/bin/env bats

setup() {
  SCRIPT="${BATS_TEST_DIRNAME}/resolve-publish-metadata"
  REPO="$(mktemp -d)"
  MOCK_DIR="$(mktemp -d)"
  export PATH="${MOCK_DIR}:${PATH}"

  git -C "${REPO}" init -q -b main
  git -C "${REPO}" config user.name Test
  git -C "${REPO}" config user.email test@example.com
  mkdir -p "${REPO}/apps/node"
  printf '%s\n' '{"name":"node-app","version":"1.2.3"}' >"${REPO}/apps/node/package.json"
  git -C "${REPO}" add .
  git -C "${REPO}" commit -qm root

  export MOON_MOCK_OUTPUT='{"projects":[{"id":"node-app","source":"apps/node","tasks":{"publish":{}}},{"id":"private-app","source":"apps/node","tasks":{"test":{}}}]}'
  cat >"${MOCK_DIR}/moon" <<'EOF'
#!/usr/bin/env bash
printf '%s\n' "${MOON_MOCK_OUTPUT}"
EOF
  chmod +x "${MOCK_DIR}/moon"
  cd "${REPO}"
}

teardown() {
  rm -rf "${REPO}" "${MOCK_DIR}"
}

@test "latest uses source version and Moon project ID release tag" {
  run bash "${SCRIPT}" node-app latest main 1
  [ "$status" -eq 0 ]
  run jq -e '.target == "node-app" and .source == "apps/node" and .version == "1.2.3" and .release_tag == "node-app-v1.2.3"' <<<"$output"
  [ "$status" -eq 0 ]
}

@test "main next bumps minor from component stable tag and appends attempt" {
  git tag node-app-v1.2.3
  git commit -q --allow-empty -m one
  git commit -q --allow-empty -m two

  run bash "${SCRIPT}" node-app next main 3
  [ "$status" -eq 0 ]
  run jq -e '.stable_tag == "node-app-v1.2.3" and .commit_distance == 2 and .version == "1.3.0-next.2.3"' <<<"$output"
  [ "$status" -eq 0 ]
}

@test "release next bumps patch and excludes prerelease tags" {
  git tag node-app-v1.2.3
  git commit -q --allow-empty -m one
  git tag node-app-v9.0.0-next.1

  run bash "${SCRIPT}" node-app next release/1.2 1
  [ "$status" -eq 0 ]
  run jq -e '.stable_tag == "node-app-v1.2.3" and .version == "1.2.4-next.1.1"' <<<"$output"
  [ "$status" -eq 0 ]
}

@test "missing component tag uses legacy source tag then repository root" {
  git tag apps/node-v1.2.3
  git commit -q --allow-empty -m one

  run bash "${SCRIPT}" node-app next main 1
  [ "$status" -eq 0 ]
  run jq -e '.stable_tag == "apps/node-v1.2.3" and .commit_distance == 1' <<<"$output"
  [ "$status" -eq 0 ]

  git tag -d apps/node-v1.2.3 >/dev/null
  run bash "${SCRIPT}" node-app next main 1
  [ "$status" -eq 0 ]
  run jq -e '.stable_tag == null and .commit_distance == 2' <<<"$output"
  [ "$status" -eq 0 ]
}


@test "rejects unknown non-publishable branch and malformed version" {
  run bash "${SCRIPT}" missing latest main 1
  [ "$status" -ne 0 ]

  run bash "${SCRIPT}" private-app latest main 1
  [ "$status" -ne 0 ]

  run bash "${SCRIPT}" node-app next feature/nope 1
  [ "$status" -ne 0 ]

  printf '%s\n' '{"name":"node-app","version":"banana"}' >apps/node/package.json
  run bash "${SCRIPT}" node-app latest main 1
  [ "$status" -ne 0 ]
}
