# Create one runtime per configured Plugin

**Status:** accepted

AgentBoard creates one runtime for each configured Source and Action during Workspace loading. Every Plugin Descriptor exposes one `runtime` callback. The Plugin role selects the returned runtime interface.

A Source runtime collects normalized Items. An Action runtime executes one Rendered Action at a time. AgentBoard renders the Action inputs before execution and passes them to the Action runtime with the Item and runtime context.

Source and Action runtime creation can return synchronous or asynchronous results. A runtime creation error fails Workspace loading. Source collection errors remain Source-scoped. Action rendering and execution errors remain Item-scoped.

Watch Mode reuses the loaded Workspace and all Plugin runtimes for every Run cycle. Configuration changes require a command restart. Health checks use validated Plugin configuration without creating runtimes.

Explicit runtime disposal remains outside issue #52 because process exit releases runtime resources.
