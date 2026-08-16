# 0009 Coordinate releases with Release Please and Moon

## Status

Accepted

## Context

This repository publishes independently configured Moon projects. Release Please owns stable version gates and GitHub releases, while Moon owns project discovery, affected-project selection, and project-specific publish tasks. GitHub Actions connects those systems.

Without one contract, repositories drift in three places: whether ordinary commits produce prereleases, where versions are computed, and whether the publish workflow acts on the same commit selected by the release workflow.

The workflow must support production and hotfix branches plus manual prerelease publication. Repository-specific package paths and publish commands may differ; orchestration semantics must remain portable.

## Decision

Use `release.yml` as the release coordinator and `publish.yml` as a payload-driven executor.

### Release gates

- Pushes to `main` use the normal Release Please configuration.
- Pushes to `release/*` use the hotfix Release Please configuration.
- `releases_created=true` publishes stable builds with `PUBLISH_TAG=latest`.
- `releases_created=false` always publishes prerelease builds with `PUBLISH_TAG=next`, including runs that create or update a Release Please pull request.
- `prs_created` is reporting data only and never suppresses publication.
- A `release/*` branch starts at a stable release commit already processed from `main`. The release branch skips that root commit.
- Commits added after the release-branch root are owned by the hotfix configuration.
- A later merge of hotfix-owned history into `main` is not processed again by the normal configuration. Ownership detection uses the associated pull request head branch, with Git ancestry as fallback, so merge, squash, and ordinary merge-back paths remain covered.

### Project selection

Automatic push runs select publishable Moon projects affected by the pushed commit range.

Manual runs support:

- `changed` (default): projects changed on the selected branch since its fork point from the workspace default branch;
- `all`: every Moon project with a `publish` task;
- `projects`: comma-separated Moon project IDs.

Unknown projects and projects without a `publish` task are skipped and reported in the GitHub step summary. They do not fail the workflow. Operational failures still fail the workflow.

### Versions

- Stable versions come from the Release Please manifest at the selected source commit.
- Prerelease versions use `<next-stable>-next.<run_number>`.
- The normal release policy computes the next stable minor version.
- The hotfix release policy computes the next stable patch version.
- Retry attempts reuse the same version. Publish tasks must therefore tolerate or clearly report an already-published version.

### Dispatch contract

The coordinator dispatches one `publish-package` event per selected project:

```json
{
  "project_id": "example",
  "version": "1.3.0-next.481",
  "publish_tag": "next",
  "release_tag": "1.3.0-next.481",
  "source_sha": "abc123"
}
```

`release_tag` is the actual GitHub release tag. Stable builds receive Release Please's component tag; prerelease builds use the computed prerelease version as the tag.

`publish.yml` checks out `source_sha`, exports the payload as `PUBLISH_TARGET`, `PUBLISH_VERSION`, `PUBLISH_TAG`, and `PUBLISH_RELEASE_TAG`, then runs `moon run "${PUBLISH_TARGET}:publish" --force`. It creates a missing prerelease before publishing, but does not select projects, compute versions, or decide release eligibility.

### Ownership

The `.github` directory is a Moon project. Its tests cover release helper scripts and track this ADR as an input. Repositories copying this decision should keep orchestration and tests in their equivalent GitHub automation project; only package configuration and project publish tasks should vary.

## Consequences

- Every non-release push can produce prerelease artifacts.
- Stable and prerelease publication share one dispatch contract.
- Published artifacts are tied to an immutable source commit.
- Manual selection mistakes remain visible without blocking valid selected projects.
- Release helpers become testable through the repository's normal Moon task graph.
- Repositories must keep Release Please manifest paths aligned with Moon project source paths.