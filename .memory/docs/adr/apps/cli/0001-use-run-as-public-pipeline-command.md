# Use `run` as the public pipeline command

AgentBoard's public command is `run`, not `collect`, because users trigger a full workflow: read sources, update the store, and execute pending source-owned actions. Collection remains an internal stage of a run so the CLI does not expose a half-step that conflicts with the product model.
