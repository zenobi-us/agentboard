# Use `search-query-parser` with AgentBoard field evaluation

AgentBoard uses `search-query-parser` for boolean query structure and evaluates `field:value` terms against item frontmatter itself. This preserves user-facing AND/OR/NOT/parentheses syntax without inventing a boolean parser, while keeping field matching tied to AgentBoard's item model.
