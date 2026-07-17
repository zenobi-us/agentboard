#!/usr/bin/env bats

load test_helper

setup() { setup_agentboard_test; }
teardown() { teardown_agentboard_test; }

@test "workspace list and compatibility alias return named configs in order" {
  run "$AB" workspace list
  [ "$status" -eq 0 ]
  [ -z "$output" ]

  mkdir -p "$XDG_CONFIG_HOME/agentboard"
  touch "$XDG_CONFIG_HOME/agentboard/zeta.toml"
  touch "$XDG_CONFIG_HOME/agentboard/alpha.toml"
  touch "$XDG_CONFIG_HOME/agentboard/ignored.txt"

  run "$AB" workspace list
  [ "$status" -eq 0 ]
  [ "$output" = $'alpha\nzeta' ]

  run "$AB" workspaces
  [ "$status" -eq 0 ]
  [ "$output" = $'alpha\nzeta' ]
}

@test "workspace init creates an empty workspace and refuses overwrite" {
  run "$AB" workspace init work

  [ "$status" -eq 0 ]
  [ "$output" = "$XDG_CONFIG_HOME/agentboard/work.toml" ]
  [ "$(cat "$XDG_CONFIG_HOME/agentboard/work.toml")" = "sources = []" ]

  run "$AB" workspace init work
  [ "$status" -ne 0 ]
  [[ "$output" == *"already exists"* ]]
  [ "$(cat "$XDG_CONFIG_HOME/agentboard/work.toml")" = "sources = []" ]
}

@test "workspace init rejects unsafe names" {
  for name in "bad/name" "bad name" "." "../escape"; do
    run "$AB" workspace init "$name"
    [ "$status" -ne 0 ]
    [[ "$output" == *"workspace name must contain only"* ]]
  done

  [ ! -e "$XDG_CONFIG_HOME/escape.toml" ]
}