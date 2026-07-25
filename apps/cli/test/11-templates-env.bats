#!/usr/bin/env bats

load test_helper

setup() { setup_agentboard_test; }
teardown() { teardown_agentboard_test; }

@test "templates expose reference identity complete source raw data and action environment" {
  cat > "$QMD_ITEMS/AB-1.md" <<'EOF'
---
id: "AB-1"
title: "Fix Login!"
status: "ready"
meta:
  owner: "kin"
---
Detailed body
EOF
  export OUTPUT_FILE="$TMP/rendered"
  unset AGENTBOARD_SOURCE_ID AGENTBOARD_ITEM_ID
  cat > "$TMP/workspace.toml" <<'EOF'
[[sources]]
id = "md"
[sources.source]
kind = "qmd"
collections = ["test"]
query = "ready"

[[sources.actions]]
uses = "agentboard/run-cmd"
[sources.actions.with]
cmd = '''printf '%s' "{{ workspace.id }}|{{ source.id }}|{{ source.source.kind }}|{{ source.source.collections[0] }}|{{ source.actions[0].uses }}|{{ item.id }}|{{ item.reference_id }}|{{ item.title | slugify }}|{{ action.uses }}|{{ action.index }}|{{ item.raw.frontmatter.meta.owner }}|$AGENTBOARD_SOURCE_ID|$AGENTBOARD_ITEM_ID" > "$OUTPUT_FILE"'''
EOF

  run "$AB" run "$TMP/workspace.toml"

  [ "$status" -eq 0 ]
  local rendered
  rendered="$(cat "$OUTPUT_FILE")"
  local prefix="workspace-"
  [[ "$rendered" == "$prefix"*"|md|qmd|test|agentboard/run-cmd|$QMD_ITEMS/AB-1.md|AB-1|fix-login|agentboard/run-cmd|0|kin|md|$QMD_ITEMS/AB-1.md" ]]
}

@test "run-cmd leaves shell variables for the shell after applying cwd" {
  write_item AB-1
  mkdir -p "$TMP/launch" "$TMP/action"
  export SHELL_VALUE="agentboard"
  cat > "$TMP/workspace.toml" <<EOF
[[sources]]
id = "md"
[sources.source]
kind = "qmd"
collections = ["test"]
query = "ready"

[[sources.actions]]
uses = "agentboard/run-cmd"
[sources.actions.with]
cwd = "$TMP/action"
cmd = '''SHELL_VALUE=shell; printf '%s|%s|%s|%s' "{{ item.reference_id }}" "\$PWD" "\$SHELL_VALUE" "\${SHELL_VALUE}" > "$TMP/result"'''
EOF

  cd "$TMP/launch"
  run "$AB" run "$TMP/workspace.toml"

  [ "$status" -eq 0 ]
  [ "$(cat "$TMP/result")" = "AB-1|$TMP/action|shell|shell" ]
}

@test "template inputs expand home and environment paths after MiniJinja rendering" {
  write_item AB-1
  export HOME="$TMP/home"
  export ROOT_VAR="$TMP/root"
  export VALUE_VAR="expanded-value"
  mkdir -p "$HOME/work" "$ROOT_VAR/work"
  cat > "$TMP/workspace.toml" <<EOF
[[sources]]
id = "md"
[sources.source]
kind = "qmd"
collections = ["test"]
query = "ready"

[[sources.actions]]
uses = "agentboard/run-cmd"
[sources.actions.with]
cwd = "~/work"
cmd = '''printf '%s|%s' "\$(pwd)" "\$VALUE_VAR" > "$TMP/home-result"'''

[[sources.actions]]
uses = "agentboard/run-cmd"
[sources.actions.with]
cwd = "\${ROOT_VAR}/work"
cmd = '''printf '%s|%s' "\$(pwd)" "\${VALUE_VAR}" > "$TMP/var-result"'''
EOF

  run "$AB" run "$TMP/workspace.toml"

  [ "$status" -eq 0 ]
  [ "$(cat "$TMP/home-result")" = "$HOME/work|expanded-value" ]
  [ "$(cat "$TMP/var-result")" = "$ROOT_VAR/work|expanded-value" ]
}