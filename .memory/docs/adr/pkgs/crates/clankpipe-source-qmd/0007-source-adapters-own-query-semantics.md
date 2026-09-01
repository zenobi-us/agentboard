# Source adapters own query semantics

AgentBoard source queries belong to each Source adapter, not to a shared AgentBoard query evaluator. This supersedes ADR-0003.

The QMD Source passes its configured query to `qmd query` and requires the workspace to name one or more QMD collections. AgentBoard only normalizes returned documents into Items and stores the raw QMD result plus retrieved document content.

This keeps Source-specific search features behind the Source seam. It also avoids forcing Jira, GitHub, Linear, and QMD into one lowest-common-denominator query language.
