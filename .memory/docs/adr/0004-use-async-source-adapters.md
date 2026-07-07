# Use async source adapters

AgentBoard defines source adapters as async even though the MVP only ships a markdown source. Network-backed sources such as GitHub, Jira, and Linear are core to the product direction, so accepting Tokio and async adapter boundaries now avoids rewriting the adapter interface after the first non-local source lands.
