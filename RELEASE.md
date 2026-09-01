# ClankPipe Release Process

ClankPipe uses Release Please to manage version bumps and changelog pull requests.

## Channels

- **Pre-release**: normal PRs to `main` create `x.x.x-next.J` release PRs.
- **Stable**: merged release PRs cut the final version.

## Publish

GitHub release assets are produced by the `publish.yml` workflow.

- release event: publish from the tagged commit
- manual run: choose a moon project id, usually `cli`

The publish task lives in the project `moon.yml`, so each app/pkg controls its own build or packaging steps.

## Compatibility

The `agentboard` executable and legacy configuration, Store, and Plugin paths remain available through the 0.x release line. The project will remove these compatibility aliases in the first stable major release.

## Notes

- Keep release metadata in `release-please-config--release.json`, `release-please-config--hotfix.json`, and `.release-please-manifest.json` in sync.
- Use conventional commits.
