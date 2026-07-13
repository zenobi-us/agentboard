#!/usr/bin/env bats

setup() {
  SCRIPT="${BATS_TEST_DIRNAME}/release-branches.sh"
  REPO="$(mktemp -d)"

  git -C "${REPO}" init -q -b main
  git -C "${REPO}" config user.name Test
  git -C "${REPO}" config user.email test@example.com

  git -C "${REPO}" commit -q --allow-empty -m a
  git -C "${REPO}" commit -q --allow-empty -m b
  ROOT="$(git -C "${REPO}" rev-parse HEAD)"
  git -C "${REPO}" switch -q -c release/1.x
  git -C "${REPO}" commit -q --allow-empty -m c
  RELEASE_C="$(git -C "${REPO}" rev-parse HEAD)"
  git -C "${REPO}" commit -q --allow-empty -m d
  RELEASE_D="$(git -C "${REPO}" rev-parse HEAD)"
  git -C "${REPO}" switch -q main
  git -C "${REPO}" commit -q --allow-empty -m e
}

teardown() {
  rm -rf "${REPO}"
}

@test "get-commits prints root then release commits oldest first" {
  cd "${REPO}"

  run bash "${SCRIPT}" get-commits release/1.x main

  [ "$status" -eq 0 ]
  [ "$output" = "${ROOT}
${RELEASE_C}
${RELEASE_D}" ]
}

@test "is-root-commit accepts the root commit" {
  cd "${REPO}"

  run bash "${SCRIPT}" is-root-commit "${ROOT}" release/1.x main

  [ "$status" -eq 0 ]
  [ -z "$output" ]
}

@test "is-root-commit rejects a later release commit" {
  cd "${REPO}"

  run bash "${SCRIPT}" is-root-commit "${RELEASE_C}" release/1.x main

  [ "$status" -eq 1 ]
  [ -z "$output" ]
}

@test "is-hotfix-commit accepts release branch commits after the root" {
  cd "${REPO}"

  run bash "${SCRIPT}" is-hotfix-commit "${RELEASE_C}" release/1.x main

  [ "$status" -eq 0 ]
}

@test "is-hotfix-commit rejects the already released branch root" {
  cd "${REPO}"

  run bash "${SCRIPT}" is-hotfix-commit "${ROOT}" release/1.x main

  [ "$status" -eq 1 ]
}

@test "is-hotfix-merge recognizes a main merge containing hotfix history" {
  cd "${REPO}"
  git update-ref refs/remotes/origin/release/1.x "${RELEASE_D}"
  git update-ref refs/remotes/origin/main main
  git merge -q --no-ff release/1.x -m "merge hotfix"
  merge_commit="$(git rev-parse HEAD)"

  run bash "${SCRIPT}" is-hotfix-merge "${merge_commit}" origin/main

  [ "$status" -eq 0 ]
}

@test "rejects non-release branch names" {
  cd "${REPO}"

  run bash "${SCRIPT}" get-commits main

  [ "$status" -eq 1 ]
  [[ "$output" == *"does not match pattern"* ]]
}

@test "reports unknown refs" {
  cd "${REPO}"

  run bash "${SCRIPT}" get-commits release/missing main

  [ "$status" -eq 1 ]
  [[ "$output" == *"Unknown commit or branch: release/missing"* ]]
}
