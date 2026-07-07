# Store items and actions as per-source JSONL

AgentBoard stores item observations and action attempts as append-only JSONL files under each source directory. This keeps the MVP inspectable and avoids shared write contention when source pipelines run concurrently, while preserving enough history to derive latest item state and retry failed actions.
