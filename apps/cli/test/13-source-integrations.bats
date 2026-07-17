#!/usr/bin/env bats

load test_helper

setup() { setup_agentboard_test; }
teardown() { teardown_agentboard_test; }

@test "live QMD source collects normalized items" {
  [ -n "${AGENTBOARD_TEST_QMD_COLLECTION:-}" ] || skip "set AGENTBOARD_TEST_QMD_COLLECTION"
  [ -n "${AGENTBOARD_TEST_QMD_QUERY:-}" ] || skip "set AGENTBOARD_TEST_QMD_QUERY"
  PATH="$ORIGINAL_PATH" command -v qmd >/dev/null || skip "real qmd not installed"
  export PATH="$ORIGINAL_PATH"
  cat > "$TMP/workspace.toml" <<EOF
[[sources]]
id = "qmd-live"
[sources.source]
kind = "qmd"
collections = ["$AGENTBOARD_TEST_QMD_COLLECTION"]
query = '''$AGENTBOARD_TEST_QMD_QUERY'''
limit = 1
EOF

  run "$AB" --color never run "$TMP/workspace.toml"
  [ "$status" -eq 0 ]
  run "$AB" list "$TMP/workspace.toml" --json
  [ "$status" -eq 0 ]
  printf '%s' "$output" | /usr/bin/python3 -c 'import json,sys; rows=json.load(sys.stdin); assert rows; assert all(row["item"]["source_kind"] == "qmd" for row in rows)'
}

@test "live GitHub source collects issue identities and respects limit" {
  [ -n "${AGENTBOARD_TEST_GITHUB_TOKEN:-}" ] || skip "set AGENTBOARD_TEST_GITHUB_TOKEN"
  [ -n "${AGENTBOARD_TEST_GITHUB_QUERY:-}" ] || skip "set AGENTBOARD_TEST_GITHUB_QUERY"
  export PATH="$ORIGINAL_PATH"
  cat > "$TMP/github-token" <<'EOF'
#!/bin/bash
printf '%s' "$AGENTBOARD_TEST_GITHUB_TOKEN"
EOF
  chmod +x "$TMP/github-token"
  cat > "$TMP/workspace.toml" <<EOF
[[sources]]
id = "github-live"
[sources.source]
kind = "github"
mode = "issue"
query = '''$AGENTBOARD_TEST_GITHUB_QUERY'''
limit = 1
[sources.source.credentials]
helper = "$TMP/github-token"
[sources.source.status_map]
"ready" = "ready"
EOF

  run "$AB" --color never run "$TMP/workspace.toml"
  [ "$status" -eq 0 ]
  run "$AB" list "$TMP/workspace.toml" --json
  [ "$status" -eq 0 ]
  printf '%s' "$output" | /usr/bin/python3 -c 'import json,sys; rows=json.load(sys.stdin); assert 0 < len(rows) <= 1; item=rows[0]["item"]; assert item["source_kind"] == "github"; assert "#" in item["id"]'
}

@test "live Jira source collects normalized issues and respects limit" {
  [ -n "${AGENTBOARD_TEST_JIRA_SITE:-}" ] || skip "set AGENTBOARD_TEST_JIRA_SITE"
  [ -n "${AGENTBOARD_TEST_JIRA_EMAIL:-}" ] || skip "set AGENTBOARD_TEST_JIRA_EMAIL"
  [ -n "${AGENTBOARD_TEST_JIRA_TOKEN:-}" ] || skip "set AGENTBOARD_TEST_JIRA_TOKEN"
  [ -n "${AGENTBOARD_TEST_JIRA_JQL:-}" ] || skip "set AGENTBOARD_TEST_JIRA_JQL"
  export PATH="$ORIGINAL_PATH"
  export JIRA_EMAIL="$AGENTBOARD_TEST_JIRA_EMAIL"
  export JIRA_API_TOKEN="$AGENTBOARD_TEST_JIRA_TOKEN"
  cat > "$TMP/workspace.toml" <<EOF
[[sources]]
id = "jira-live"
[sources.source]
kind = "jira"
site = "$AGENTBOARD_TEST_JIRA_SITE"
jql = '''$AGENTBOARD_TEST_JIRA_JQL'''
limit = 1
EOF

  run "$AB" --color never run "$TMP/workspace.toml"
  [ "$status" -eq 0 ]
  run "$AB" list "$TMP/workspace.toml" --json
  [ "$status" -eq 0 ]
  printf '%s' "$output" | /usr/bin/python3 -c 'import json,sys; rows=json.load(sys.stdin); assert 0 < len(rows) <= 1; item=rows[0]["item"]; assert item["source_kind"] == "jira"; assert item["id"]; assert item["url"]'
}

@test "live GitHub source rejects invalid credentials" {
  [ "${AGENTBOARD_TEST_LIVE_NEGATIVE:-0}" = 1 ] || skip "set AGENTBOARD_TEST_LIVE_NEGATIVE=1"
  [ -n "${AGENTBOARD_TEST_GITHUB_QUERY:-}" ] || skip "set AGENTBOARD_TEST_GITHUB_QUERY"
  export PATH="$ORIGINAL_PATH"
  cat > "$TMP/workspace.toml" <<EOF
[[sources]]
id = "github-invalid"
[sources.source]
kind = "github"
mode = "issue"
query = '''$AGENTBOARD_TEST_GITHUB_QUERY'''
limit = 1
[sources.source.credentials]
helper = "printf definitely-invalid-agentboard-token"
[sources.source.status_map]
"ready" = "ready"
EOF

  run "$AB" --color never doctor "$TMP/workspace.toml"

  [ "$status" -ne 0 ]
  [[ "$output" == *"github issue search failed with"* ]]
}