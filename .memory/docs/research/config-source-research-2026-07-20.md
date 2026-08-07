# Research: Where AgentBoard reads configuration from

## Summary

AgentBoard reads one TOML Workspace. When an operational command omits its Workspace argument, AgentBoard reads `./.agentboard.toml` from the process current directory. It does not search parent directories. A supplied Workspace continues to select either a named config under the platform user config directory or an explicit path. [`apps/cli/src/cli.rs:48-75`](../../../apps/cli/src/cli.rs#L48-L75) [`apps/cli/src/config.rs:65-109`](../../../apps/cli/src/config.rs#L65-L109) [`apps/cli/docs/workspaces.md:9-37`](../../../apps/cli/docs/workspaces.md#L9-L37)

## Selection rules

1. **No Workspace argument** — `run`, `list`, and `doctor` accept no positional Workspace. The watch mode uses `run --watch`. `show ITEM_ID` also omits it. The loader selects the relative path `.agentboard.toml`, so filesystem resolution is against the process current directory. [`apps/cli/src/cli.rs:48-75`](../../../apps/cli/src/cli.rs#L48-L75) [`apps/cli/src/cli.rs:126-135`](../../../apps/cli/src/cli.rs#L126-L135) [`apps/cli/src/config.rs:101-108`](../../../apps/cli/src/config.rs#L101-L108)

2. **Explicit path** — an argument ending in `.toml` or containing `/` is expanded and read directly. Relative paths resolve from the current directory. Leading `~/`, `$VAR`, and `${VAR}` expansion remains supported by `expand_path`. [`apps/cli/src/config.rs:101-108`](../../../apps/cli/src/config.rs#L101-L108) [`apps/cli/src/config.rs:310-330`](../../../apps/cli/src/config.rs#L310-L330)

3. **Named Workspace** — any other supplied argument resolves to `BaseDirs::config_dir()/agentboard/<name>.toml`. The checked-in docs show the Unix example `~/.config/agentboard/work.toml`. [`apps/cli/src/config.rs:34-39`](../../../apps/cli/src/config.rs#L34-L39) [`apps/cli/src/config.rs:101-108`](../../../apps/cli/src/config.rs#L101-L108) [`apps/cli/docs/workspaces.md:19-29`](../../../apps/cli/docs/workspaces.md#L19-L29)

4. **Precedence** — supplying a name or path bypasses `.agentboard.toml`; there is no fallback after a supplied file fails. Only one selected file is read and deserialized. There is no layered merge. [`apps/cli/src/config.rs:80-109`](../../../apps/cli/src/config.rs#L80-L109)

5. **No ancestor discovery** — the default is the literal relative path `.agentboard.toml`; no implementation walks parent directories. The user-facing docs state this explicitly. [`apps/cli/src/config.rs:101-108`](../../../apps/cli/src/config.rs#L101-L108) [`apps/cli/docs/workspaces.md:11-17`](../../../apps/cli/docs/workspaces.md#L11-L17)

## Command forms

```text
agentboard run [WORKSPACE]
agentboard run [WORKSPACE] --watch
agentboard list [WORKSPACE]
agentboard doctor [WORKSPACE]
agentboard show ITEM_ID
agentboard show WORKSPACE ITEM_ID
```

All forms dispatch through the same loader. `doctor` skips immediate semantic validation so its checks can report validation failures itself. [`apps/cli/src/cli.rs:148-184`](../../../apps/cli/src/cli.rs#L148-L184) [`apps/cli/src/config.rs:71-86`](../../../apps/cli/src/config.rs#L71-L86)

## Workspace identity

Named Workspace ids remain the supplied name. Default and explicit-path Workspaces use the file stem plus the first 12 hexadecimal characters of a SHA-256 hash of the canonical path. Thus a local `.agentboard.toml` receives an id shaped like `.agentboard-<hash>`. [`apps/cli/src/config.rs:88-98`](../../../apps/cli/src/config.rs#L88-L98) [`apps/cli/src/config.rs:337-340`](../../../apps/cli/src/config.rs#L337-L340)

## Verification

The repository has unit coverage for omitted Workspace selection, unchanged named/path classification, optional operational command arguments, and the one-or-two-position `show` syntax. A smoke test also loaded a temporary cwd `.agentboard.toml` with both `run --dry-run` and `doctor`. [`apps/cli/src/config.rs:350-372`](../../../apps/cli/src/config.rs#L350-L372) [`apps/cli/src/cli.rs:195-249`](../../../apps/cli/src/cli.rs#L195-L249)
