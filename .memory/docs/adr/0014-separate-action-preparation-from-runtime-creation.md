# Separate Action preparation from Action runtime creation

**Status:** accepted

AgentBoard will prepare each configured Action once during Workspace loading. An Action Plugin will expose `prepare`, which returns a Prepared Action and can allocate shared resources. After AgentBoard renders inputs for one Item, the Prepared Action will create one Action runtime for that Rendered Action. Source runtime creation and Action preparation can return synchronous or asynchronous results.

Preparation errors will fail Workspace loading. Action runtime creation and execution errors will remain Item-scoped. Health checks will use validated Plugin configuration without a Prepared Action.

Watch Mode will load one Workspace and reuse its Source runtimes and Prepared Actions for every Run cycle. Configuration changes will require a command restart. Explicit resource disposal remains outside issue #52 because process exit releases the resources.
