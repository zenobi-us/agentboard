# Store items and actions as append-only JSONL

AgentBoard stores item observations and action attempts as append-only JSONL. This keeps the MVP inspectable and avoids shared write contention when source pipelines run concurrently, while preserving enough history to derive latest item state and retry failed actions.

Revision: Store partitioning is no longer per source directory. Item observations are partitioned by `source.slug`, the upstream item universe. Action attempts are partitioned by `source.slug` plus `source.hash`, the configured Source view and Action plan. This keeps Jira issue keys scoped to their Jira site while avoiding duplicate item Stores for multiple JQL views of the same site.
