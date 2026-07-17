#!/usr/bin/env bats

load test_helper

setup() { setup_agentboard_test; }
teardown() { teardown_agentboard_test; }

@test "root help exposes the supported command surface" {
  run "$AB" --help

  [ "$status" -eq 0 ]
  for command in workspace run watch list show doctor schema; do
    [[ "$output" == *"$command"* ]]
  done
  [[ "$output" != *"workspaces"* ]]
}

@test "every public command has help" {
  for command in workspace run watch list show doctor schema; do
    run "$AB" "$command" --help
    [ "$status" -eq 0 ]
    [[ "$output" == *"Usage:"* ]]
  done

  run "$AB" workspace init --help
  [ "$status" -eq 0 ]
  run "$AB" workspace list --help
  [ "$status" -eq 0 ]
}

@test "global flags work before and after subcommands" {
  run "$AB" --color never workspace list
  [ "$status" -eq 0 ]

  run "$AB" workspace list --color never
  [ "$status" -eq 0 ]

  run "$AB" workspace --color never list
  [ "$status" -eq 0 ]
}

@test "invalid commands and invalid watch intervals fail" {
  run "$AB" unsupported
  [ "$status" -ne 0 ]
  [[ "$output" == *"unrecognized subcommand"* ]]

  printf 'sources = []\n' > "$TMP/empty.toml"
  run "$AB" watch "$TMP/empty.toml" --interval nope
  [ "$status" -ne 0 ]
}