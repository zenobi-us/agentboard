# Derive Store identity from resolved Plugin identity

**Status:** accepted

The Bun runtime will derive an Item Bucket identity from the resolved Source Plugin identity and the Plugin-provided Item Bucket identity. It will derive a configured Source identity from the Source ID, resolved Source Plugin identity, and normalized Source configuration. Actions will not affect either identity.

External Plugins will use the exact package name as their Plugin identity. Inline Plugins will use the configuration path, role, and position. A package rename or inline Plugin identity change will create a new Store identity. AgentBoard will not connect the new Store records to the old records.