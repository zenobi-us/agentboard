# Sources (/cli/sources)



# Sources [#sources]

A Source reads items from an external or local system.

This CLI doc only covers how Sources fit into a Workspace.

For Source-specific config and behavior, see the specific source docs.

## Workspace Shape [#workspace-shape]

```toml
[[sources]]
id = "local"

[sources.source]
uses = "@agentboard/source-qmd"
# source-specific fields live here

[[sources.actions]]
uses = "@agentboard/action-run-cmd"

[sources.actions.with]
cmd = "echo {{ item.reference_id }}"
```

## Execution Order [#execution-order]

A workspace can define multiple Sources, and they can be sources of the same or different kinds.

They are run concurrently, and when a source has finished retrieving items, its Actions are run sequentially for each item.

## Normalized Item Shape [#normalized-item-shape]

After downloading or reading items from a source, agentboard normalizes them into a small model that is stored in the Workspace store.

They are stored in a common shape, so Actions can be written to work with any Source.

Every Source produces normalized items:

* `id`
* `reference_id`
* `title`
* `status`
* `url`
* `source_id`
* `source_kind`
* `raw`

`raw` keeps the original Source payload so the normalized model can stay small.

`id` is the adapter-owned, collision-resistant identity used by the Store and
Action retry checks. `reference_id` is the provider-facing identifier intended
for templates, such as GitHub `10` or Jira `ABC-123`.

For Sources with field mapping, the existing `field_map.id` or `map.id` setting
selects `reference_id`. It never changes `id`.

## Source Docs [#source-docs]

* [QMD source](/sources/qmd)
* [Jira source](/sources/jira)
* [GitHub source](/sources/github)

## CLI-Owned Behavior [#cli-owned-behavior]

The CLI validates Source ids before a Run:

* Source ids must be nonempty.
* Source ids must be unique within one Workspace.

Source packages own Source-specific validation and collection behavior.
