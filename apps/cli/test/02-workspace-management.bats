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

@test "workspace edit opens invalid TOML with fixed editor arguments" {
  mkdir -p "$XDG_CONFIG_HOME/agentboard"
  printf 'invalid = [' > "$XDG_CONFIG_HOME/agentboard/work.toml"
  cat > "$TMP/bin/editor-fixture" <<'EOF'
#!/bin/bash
printf '%s\n' "$@" > "$EDITOR_LOG"
exit "${EDITOR_EXIT:-0}"
EOF
  chmod +x "$TMP/bin/editor-fixture"
  export EDITOR_LOG="$TMP/editor.log"
  export EDITOR="$TMP/bin/editor-fixture --wait"

  run "$AB" workspace edit work

  [ "$status" -eq 0 ]
  [ "$output" = "" ]
  [ "$(cat "$EDITOR_LOG")" = $'--wait\n'"$XDG_CONFIG_HOME/agentboard/work.toml" ]
}

@test "workspace edit reports editor and workspace failures" {
  mkdir -p "$XDG_CONFIG_HOME/agentboard"
  printf 'sources = []\n' > "$XDG_CONFIG_HOME/agentboard/work.toml"

  unset EDITOR
  run "$AB" workspace edit work
  [ "$status" -ne 0 ]
  [[ "$output" == *"EDITOR"* ]]

  export EDITOR="   "
  run "$AB" workspace edit work
  [ "$status" -ne 0 ]
  [[ "$output" == *"EDITOR"* ]]

  export EDITOR="$TMP/bin/editor-never-runs"
  run "$AB" workspace edit missing
  [ "$status" -ne 0 ]
  [[ "$output" == *"workspace does not exist"* ]]
  [ ! -e "$XDG_CONFIG_HOME/agentboard/missing.toml" ]

  run "$AB" workspace edit work
  [ "$status" -ne 0 ]
  [[ "$output" == *"start editor"* ]]

  cat > "$TMP/bin/editor-fails" <<'EOF'
#!/bin/bash
exit 7
EOF
  chmod +x "$TMP/bin/editor-fails"
  export EDITOR="$TMP/bin/editor-fails"
  run "$AB" workspace edit work
  [ "$status" -ne 0 ]
  [[ "$output" == *"editor exited unsuccessfully"* ]]
}
