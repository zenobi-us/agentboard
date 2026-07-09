# AgentBoard GitHub Source Context

`agentboard-source-github` collects GitHub records and normalizes them into AgentBoard Items.

## Language

**GitHub source**:
A Source whose mode selects one GitHub collection API.
_Avoid_: GitHub integration

**Issue mode**:
A GitHub source mode that uses GitHub issue search query semantics and returns GitHub issues.
_Avoid_: Project issue mode

**Credential helper**:
A configured command that returns a GitHub token on stdout.
_Avoid_: Login flow

## Boundaries

- GitHub owns request construction, credential lookup, GitHub response normalization, and mode-specific query semantics.
- Issue mode query semantics belong to GitHub issue search. AgentBoard must not reinterpret the query beyond guarding against pull requests.
- Issue mode item ids are `owner/repo#number`.
- Issue mode requires configured status label mapping; when no configured label matches an issue, status falls back to the GitHub issue state.
- The normalized Item must preserve the raw GitHub issue payload.
- Duplicate normalized item ids in one source are source errors.

## ADRs

Read `.memory/docs/adr/0008-use-one-github-source-with-explicit-modes.md` before changing GitHub package boundaries, modes, identity, status, or credentials.
