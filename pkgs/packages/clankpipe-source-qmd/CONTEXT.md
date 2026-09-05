# ClankPipe QMD Source Context

`clankpipe-source-qmd` collects items from QMD collections and normalizes them into ClankPipe Items.

## Language

**QMD source**:
A Source whose query is passed to `qmd query` for one or more QMD collections.
_Avoid_: Markdown source; QMD is the integration boundary

**Document reference**:
The identifier returned by `qmd query` and used as the normalized Item identity.
_Avoid_: File path unless the returned value is actually a path

**Frontmatter mapping**:
A config-provided field path used to map QMD YAML frontmatter into normalized Item fields.
_Avoid_: Schema migration, transform pipeline

## Boundaries

- QMD owns QMD command invocation, result JSON parsing, document retrieval, and frontmatter parsing.
- QMD query semantics belong to QMD. ClankPipe must not reinterpret QMD query syntax.
- The QMD document reference is `item.id`; the mapped frontmatter id is `item.reference_id`.
- The normalized Item must include source id, source kind, minimum display fields, and raw QMD payload/document content.
- Duplicate normalized item ids in one source are source errors.

## ADRs

Read `.memory/docs/adr/pkgs/packages/clankpipe-source-qmd/` before changing query handling, frontmatter mapping, or raw payload storage.
