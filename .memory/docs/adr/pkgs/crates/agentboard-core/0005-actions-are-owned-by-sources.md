# Actions are owned by sources

AgentBoard actions are declared on a source, not globally on a workspace. This keeps action execution, retries, and result identity scoped to the items produced by that source, and avoids ambiguous global actions when multiple sources feed the same workspace.
