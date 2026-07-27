# Store observations, snapshots, and attempts as append-only JSONL

AgentBoard stores Item observations, committed Source Snapshot boundaries, and Action attempts as append-only JSONL. Each successful Source collection appends one complete Source Snapshot; only the latest complete Snapshot defines that Source's current membership and normalized Item values. Failed collections and incomplete writes leave the previous Snapshot authoritative. This preserves inspectability and crash-safe concurrent reads without introducing mutable current-state files.

A Source Snapshot is keyed by Source ID plus Source kind/config, excluding Actions. Action edits therefore reuse existing membership, while Source query or field-mapping changes require a new successful Snapshot. Legacy Item records without Snapshot boundaries remain historical and are not inferred into current Source membership.

Store partitioning is not per Source directory. Item observations remain partitioned by `source.slug`, the upstream Item Bucket. Action attempts remain partitioned by `source.slug` plus `source.hash`, the configured Source view and Action plan. Source Snapshot boundaries add configured-Source membership without duplicating the Item Bucket model.
