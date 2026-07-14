#!/usr/bin/env bats

setup() {
  RELEASE="${BATS_TEST_DIRNAME}/../workflows/release.yml"
  PUBLISH="${BATS_TEST_DIRNAME}/../workflows/publish.yml"
}

@test "release dispatch sends target channel and immutable source identity only" {
  payload="$(grep 'client-payload:' "${RELEASE}")"
  [[ "${payload}" == *'"target"'* ]]
  [[ "${payload}" == *'"channel"'* ]]
  [[ "${payload}" == *'"source_sha"'* ]]
  [[ "${payload}" == *'"source_branch"'* ]]
  [[ "${payload}" != *'"version"'* ]]
  [[ "${payload}" != *'"release_tag"'* ]]
  [[ "${payload}" != *'"tag"'* ]]
}

@test "manual publish exposes target and channel without caller metadata" {
  inputs="$(sed -n '/workflow_dispatch:/,/repository_dispatch:/p' "${PUBLISH}")"
  [[ "${inputs}" == *'target:'* ]]
  [[ "${inputs}" == *'channel:'* ]]
  [[ "${inputs}" != *'version:'* ]]
  [[ "${inputs}" != *'release_tag:'* ]]
  [[ "${inputs}" != *'source_ref:'* ]]
}

@test "source validation precedes immutable checkout and repository execution" {
  validate_line="$(grep -n 'name: ValidateSourceIdentity' "${PUBLISH}" | cut -d: -f1)"
  checkout_line="$(grep -n 'name: CheckoutImmutableSource' "${PUBLISH}" | cut -d: -f1)"
  install_line="$(grep -n 'name: InstallDependencies' "${PUBLISH}" | cut -d: -f1)"
  [ "${validate_line}" -lt "${checkout_line}" ]
  [ "${checkout_line}" -lt "${install_line}" ]
  grep -F 'ref: ${{ env.PUBLISH_REF }}' "${PUBLISH}"
  grep -F 'merge-base --is-ancestor "${PUBLISH_REF}" "${branch_ref}"' "${PUBLISH}"
  grep -F '"${GITHUB_ACTOR}" == "github-actions[bot]"' "${PUBLISH}"
}

@test "publish computes metadata after checkout and handles stable and concurrent prerelease releases" {
  grep -F 'resolve-publish-metadata' "${PUBLISH}"
  grep -F 'gh release view "${PUBLISH_RELEASE_TAG}"' "${PUBLISH}"
  grep -F 'gh release create "${PUBLISH_RELEASE_TAG}"' "${PUBLISH}"
  [ "$(grep -c 'gh release view "${PUBLISH_RELEASE_TAG}"' "${PUBLISH}")" -ge 3 ]
  grep -F 'moon run "${PUBLISH_TARGET}:publish" --force' "${PUBLISH}"
}

@test "GitHub assets use computed release tag" {
  cli_moon="${BATS_TEST_DIRNAME}/../../apps/cli/moon.yml"
  grep -F 'gh release upload "${PUBLISH_RELEASE_TAG}"' "${cli_moon}"
  ! grep -F 'gh release upload "${PUBLISH_CHANNEL}"' "${cli_moon}"
}

@test "release workflow explicitly targets current hotfix branch" {
  grep -F 'target-branch: ${{ github.ref_name }}' "${RELEASE}"
  grep -F '.github/tasks/release-branches.sh is-root-commit' "${RELEASE}"
  grep -F '.github/tasks/release-branches.sh is-hotfix-merge' "${RELEASE}"
}

@test "associated PR head branch is primary squash-merge ownership signal" {
  grep -F '"repos/${GITHUB_REPOSITORY}/commits/${GITHUB_SHA}/pulls"' "${RELEASE}"
  grep -F 'startswith("release/")' "${RELEASE}"
}

@test "release mode selects normal main and hotfix release branches" {
  mode_script="${BATS_TEST_DIRNAME}/get-release-mode"
  run bash "${mode_script}" main
  [ "$status" -eq 0 ]
  [ "$output" = "normal" ]

  run bash "${mode_script}" release/0.1
  [ "$status" -eq 0 ]
  [ "$output" = "hotfix" ]
}
