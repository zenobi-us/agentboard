# AgentBoard Jira Source Context

`agentboard-source-jira` collects Jira issues through Jira's REST API and normalizes them into AgentBoard Items.

## Language

**Jira source**:
A Source whose query is Jira JQL and whose raw records are Jira issues.
_Avoid_: Generic API source

**JQL**:
The query string sent to Jira. AgentBoard does not parse it.
_Avoid_: AgentBoard query

**Credential helper**:
A configured command that returns Jira credentials instead of reading email/token environment variables.
_Avoid_: Login flow

## Boundaries

- Jira owns REST request construction, credential lookup, field selection, and Jira response normalization.
- Jira query semantics belong to Jira. AgentBoard must not reinterpret JQL.
- Jira field selection includes defaults, fields inferred from `field_map` paths that start with `fields.`, and explicit extra `fields` entries.
- Jira uses `status_map` to normalize mapped Jira status values when configured.
- The normalized Item must preserve the raw Jira issue payload.
- Duplicate normalized item ids in one source are source errors.

## ADRs

Read `.memory/docs/adr/pkgs/crates/agentboard-source-jira/` before changing async source behavior, credential handling, or JQL/query ownership.
