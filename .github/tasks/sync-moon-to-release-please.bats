#!/usr/bin/env bats

setup() {
  SCRIPT="${BATS_TEST_DIRNAME}/../actions/sync-moon-to-release-please/action.sh"
  REPO="$(mktemp -d)"
  MOCK_DIR="$(mktemp -d)"
  export PATH="${MOCK_DIR}:${PATH}"

  mkdir -p "${REPO}/apps/node" "${REPO}/apps/rust" "${REPO}/apps/private"
  printf '%s\n' '{"version":"1.2.3"}' >"${REPO}/apps/node/package.json"
  printf '%s\n' '[package]' 'name = "rust-app"' 'version = "2.3.4"' >"${REPO}/apps/rust/Cargo.toml"
  printf '%s\n' '{}' >"${REPO}/.release-please-manifest.json"
  printf '%s\n' '{"packages":{}}' >"${REPO}/release-please-config--release.json"
  printf '%s\n' '{"packages":{}}' >"${REPO}/release-please-config--hotfix.json"

  export MOON_MOCK_OUTPUT='{"projects":[{"id":"node-app","source":"apps/node","layer":"application","tasks":{"publish":{}}},{"id":"rust-app","source":"apps/rust","layer":"application","tasks":{"publish":{}}},{"id":"private-app","source":"apps/private","tasks":{"test":{}}}]}'
  cat >"${MOCK_DIR}/moon" <<'EOF'
#!/usr/bin/env bash
printf '%s\n' "${MOON_MOCK_OUTPUT}"
EOF
  cat >"${MOCK_DIR}/cargo" <<'EOF'
#!/usr/bin/env bash
manifest="$(cd "$(dirname "$3")" && pwd -P)/Cargo.toml"
printf '{"packages":[{"manifest_path":"%s","version":"2.3.4"}]}\n' "${manifest}"
EOF
  chmod +x "${MOCK_DIR}/moon" "${MOCK_DIR}/cargo"
  cd "${REPO}"
}

teardown() {
  rm -rf "${REPO}" "${MOCK_DIR}"
}

@test "sync discovers Node and Cargo projects and preserves manifest versions" {
  printf '%s\n' '{"apps/node":"9.9.9"}' >.release-please-manifest.json

  run bash "${SCRIPT}" sync
  [ "$status" -eq 0 ]

  run jq -e '. == {"apps/node":"9.9.9","apps/rust":"2.3.4"}' .release-please-manifest.json
  [ "$status" -eq 0 ]

  for config in release-please-config--release.json release-please-config--hotfix.json; do
    run jq -e '.packages == {
      "apps/node":{"component":"node-app","group":"application","release-type":"node"},
      "apps/rust":{"component":"rust-app","group":"application","release-type":"rust"}
    }' "${config}"
    [ "$status" -eq 0 ]
  done
}

@test "check rejects wrong component metadata" {
  bash "${SCRIPT}" sync
  jq '.packages["apps/node"].component = "wrong"' release-please-config--release.json >tmp
  mv tmp release-please-config--release.json

  run bash "${SCRIPT}" check
  [ "$status" -ne 0 ]
  [[ "$output" == *"package metadata differs"* ]]
}

@test "unsupported publishable version source fails" {
  export MOON_MOCK_OUTPUT='{"projects":[{"id":"unknown","source":"apps/private","tasks":{"publish":{}}}]}'

  run bash "${SCRIPT}" sync
  [ "$status" -ne 0 ]
  [[ "$output" == *"Unsupported version source"* ]]
}

@test "missing or malformed versions fail" {
  printf '%s\n' '{}' >apps/node/package.json
  run bash "${SCRIPT}" sync
  [ "$status" -ne 0 ]

  printf '%s\n' '{"version":"banana"}' >apps/node/package.json
  run bash "${SCRIPT}" sync
  [ "$status" -ne 0 ]
  [[ "$output" == *"Malformed semantic version"* ]]
}
