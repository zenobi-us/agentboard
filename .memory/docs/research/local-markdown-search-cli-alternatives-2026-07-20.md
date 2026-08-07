# Research: frontmatter-aware local Markdown search CLI alternatives to QMD

Date: 2026-07-20

## Decision summary

**Do not add one AgentBoard source crate per Markdown CLI. Add one mapped command source and keep the dedicated QMD source.** The command source should execute an argv vector, parse JSON or JSONL, select result rows through a configurable value path, optionally enrich each row by reading its referenced Markdown document, and apply the same shared field-mapping behavior used by first-party sources. This preserves each tool's query semantics behind the Source seam while avoiding a growing set of thin subprocess adapters. [Source registration contract](../../../pkgs/crates/agentboard-core/src/registry.rs) [QMD Source definition and runtime](../../../pkgs/crates/agentboard-source-qmd/src/lib.rs) [source-query ownership ADR](../adr/pkgs/crates/agentboard-source-qmd/0007-source-adapters-own-query-semantics.md)

**This works across the candidate set, but not with arbitrary native output unchanged.** Tools already returning complete mapped rows—VaultDB, mdq, Markbase, mdbasequery, Vori, Krafna, and dotmd—need only output-path/field-path configuration. Hyalo and FMQL expose nested metadata suitable for mapping. mdvs and vlt return paths plus search evidence, so the command source must optionally load the selected Markdown file and map from parsed frontmatter. fmd returns plain paths and needs a tiny JSON wrapper before AgentBoard can consume it. A generic source unifies transport and normalization; it does not add missing search operators, upstream limits, or index lifecycle management.

**VaultDB is the strongest newly discovered candidate.** `vaultdb 1.6.1` recursively scans Markdown, parses YAML frontmatter into typed values, combines metadata predicates with body-only substring or regex predicates through `_body`, emits typed JSON with relative paths, and enforces `--limit`. It does not displace Hyalo because its body search is unranked and its field grammar does not traverse nested frontmatter maps. [VaultDB README](https://github.com/rusenbb/vaultdb/blob/v1.6.1/README.md) [body virtual fields](https://github.com/rusenbb/vaultdb/blob/v1.6.1/crates/vaultdb-core/src/record.rs#L92-L165) [crates.io API](https://crates.io/api/v1/crates/vaultdb)

**`mdq` is the strongest query contract found if Python and a Git-pinned install are acceptable.** It recursively scans Markdown into an in-memory SQLite database on every run, keeps parsed frontmatter JSON and body text in separate columns, and lets one bounded SQL query combine nested metadata predicates with body predicates. It emits JSON or NDJSON. It remains too immature for the default recommendation: version `0.1.0` is Git-only, requires Python 3.13, and has no tags or releases. [README](https://github.com/davidgasquez/mdq/blob/5b62a20d1623268036e3e9d12d14955de81a2837/README.md) [schema implementation](https://github.com/davidgasquez/mdq/blob/5b62a20d1623268036e3e9d12d14955de81a2837/mdq.py#L465-L531) [package manifest](https://github.com/davidgasquez/mdq/blob/5b62a20d1623268036e3e9d12d14955de81a2837/pyproject.toml)

**FMQL is the richer Python path when optional semantic retrieval matters.** PyPI `fmql 0.3.0` provides Cypher-shaped metadata predicates, list and comparison operators, recursive discovery, bounded newline-delimited JSON output, and a `--search` stage that can be combined with structural frontmatter filters. Its default grep backend is unranked, searches serialized frontmatter after searching the body, and cannot predicate on nested YAML paths. [FMQL README](https://github.com/buyuk-dev/fmql/blob/core-v0.3.0/README.md) [grep backend](https://github.com/buyuk-dev/fmql/blob/core-v0.3.0/packages/fmql/src/fmql/search/backends/grep.py) [PyPI](https://pypi.org/project/fmql/0.3.0/)

**Markbase is the best newly verified metadata-only Cargo candidate.** It auto-refreshes a recursive DuckDB index, supports nested paths, list predicates, full read-only `SELECT`, explicit/default limits, and JSON-by-default output. Its index deliberately stores file metadata and frontmatter but not body text, so it cannot satisfy arbitrary body search. [Markbase query design](https://github.com/flyisland/markbase/blob/v0.9.5/docs/design-docs/implemented/design-010-query-subsystem.md) [index schema](https://github.com/flyisland/markbase/blob/v0.9.5/src/db.rs#L42-L58) [crates.io API](https://crates.io/api/v1/crates/markbase)

**Keep `mdbasequery` as the npm runner-up.** It provides stronger expression-based querying over arbitrary and nested frontmatter, deterministic JSON/JSONL output, and can search raw Markdown text through expressions such as `file.raw.contains(...)`. Its defect is maturity: the current package is still `0.0.1`, with one published release and no upstream commits after February 2026. [mdbasequery README](https://github.com/intellectronica/mdbasequery/blob/v0.0.1/README.md) [npm registry](https://registry.npmjs.org/mdbasequery/0.0.1) [v0.0.1 release](https://github.com/intellectronica/mdbasequery/releases/tag/v0.0.1)

**No verified Pi package is a clean AgentBoard backend.** The closest package, `pi-knowledge`, is a Pi extension rather than a standalone executable, and its implemented search filters are file type and path—not Markdown frontmatter fields. `pi-memctx` delegates semantic/deep search back to QMD and falls back to grep. [Pi package model](https://github.com/badlogic/pi-mono/blob/main/packages/coding-agent/docs/packages.md) [`pi-knowledge` package manifest](https://github.com/nczz/pi-knowledge/blob/v0.5.2/package.json) [`pi-knowledge` search options](https://github.com/nczz/pi-knowledge/blob/v0.5.2/src/engine.ts#L86-L100) [`pi-memctx` search docs](https://github.com/weauratech/pi-memctx/blob/0dada27a2ade0b2216db6f688495712e22a1e0fc/docs/search.md)

Blunt limitation: a command source is arbitrary code execution. `run`, Watch Mode (`run --watch`), `list`, and `doctor` can execute it, it inherits the AgentBoard process environment, and a workspace can use it to do anything the user can do. Use argv arrays rather than implicit shell strings, require trusted workspace configuration, cap runtime and captured output, and treat non-zero exit, malformed JSON, invalid mappings, duplicate identities, and path escape during document enrichment as hard source failures.

## AgentBoard requirements derived from the current adapter

The current QMD source does this:

```text
workspace config
  -> qmd query (query + collections + limit)
  -> document references
  -> qmd get --full per result
  -> parse YAML frontmatter
  -> map id/title/status/url
  -> normalize Item and preserve raw result/frontmatter/body
```

The adapter requires each selected document to expose string `id`, `title`, and `status`; `url` is optional. It supports nested dot-path mappings, uses the backend document reference as `item.id`, stores the mapped frontmatter id as `item.reference_id`, and preserves backend output, frontmatter, and body in `raw`. [`pkgs/crates/agentboard-source-qmd/src/lib.rs`](../../../pkgs/crates/agentboard-source-qmd/src/lib.rs) [`pkgs/crates/agentboard-source-qmd/src/docs.md`](../../../pkgs/crates/agentboard-source-qmd/src/docs.md)

An alternative therefore needs:

1. local recursive Markdown discovery;
2. real YAML-frontmatter parsing;
3. metadata filtering, particularly fields such as `status` and `queue`;
4. body search, preferably ranked lexical search;
5. machine-readable paths and metadata;
6. bounded result count and reliable exit status;
7. headless, scriptable installation;
8. no requirement for a GUI application or hosted service.

Frontmatter merely appearing in the full-text corpus does **not** satisfy requirement 3. The CLI must parse and filter metadata as metadata.

## Command-source compatibility matrix

| Candidate | Ecosystem/install | Frontmatter and body capability | Native output | Generic command-source handling |
| --- | --- | --- | --- | --- |
| **mdvs 0.8.3** | Cargo or signed release binary | Typed nested/array SQL filters plus semantic, BM25, or hybrid search | JSON envelope with `hits[].filename`, score, lines, and chunk | Select `/hits`; enrich from the Markdown path before shared mapping |
| **Hyalo 0.20.0** | Cargo | Top-level metadata predicates plus BM25/regex body search | JSON envelope with `results[]` and parsed `properties` | Select `/results`; map directly or enrich from `file` |
| **VaultDB 1.6.1** | Cargo | Typed top-level metadata plus body-only substring/regex | Top-level JSON array with selected fields | Map directly; no document enrichment required when fields are selected |
| **mdq 0.1.0** | Python/Git | Full SQL over nested frontmatter JSON and separate body column | JSON or NDJSON projected rows | Alias/select fields and map directly |
| **FMQL 0.3.0** | PyPI | Rich top-level Cypher predicates plus optional text/semantic stages | JSONL packets with `frontmatter` | Parse JSONL and map nested packet fields |
| **Markbase 0.9.5** | Cargo | Nested/list metadata SQL; no body index | Top-level JSON array | Alias/select fields and map directly |
| **mdbasequery 0.0.1** | npm | Nested metadata expressions plus raw-content predicates | JSON or JSONL projected rows | Parse chosen format and map selected fields |
| **md-fme 0.9.8** | Cargo | Nested metadata DSL; no body search | JSON object with result rows | Select its result array and map; upstream output remains unbounded |
| **Vori 1.0.0** | npm with Bun runtime | Nested equality/array membership; separate body search | Top-level JSON array with frontmatter and body | Map directly; no native limit |
| **vlt 0.11.0** | Go | Top-level metadata clauses plus text/regex search | Top-level JSON array with path/title | Enrich from each returned Markdown path |
| **Krafna 0.5.6** | Cargo | Nested/list metadata SQL; no body search | Top-level JSON array | Map directly; upstream output remains unbounded |
| **fmd 0.1.1** | Cargo | Top-level metadata selector only | Plain path lines | Wrap paths as JSON, then enrich each Markdown document |
| **dotmd-cli 0.70.3** | npm | Opinionated workflow fields plus optional body keyword scan | JSON object containing `docs` | Select `/docs` and map directly |

## mdvs — semantic, full-text, and typed metadata search

mdvs recursively scans Markdown, infers typed frontmatter into `mdvs.toml`, builds a local Lance index, and searches in `semantic`, `fulltext` (BM25), or `hybrid` (reciprocal-rank fusion) mode. `--where` applies SQL predicates to scalar, date, array, filepath, and nested frontmatter fields in the same ranked query. `--limit` bounds hits, `--output json` is supported, and exit codes distinguish success from pipeline errors. [README](https://github.com/edochi/mdvs/blob/v0.8.3/README.md) [search command](https://github.com/edochi/mdvs/blob/v0.8.3/book/src/commands/search.md) [search guide](https://github.com/edochi/mdvs/blob/v0.8.3/book/src/search-guide.md)

```bash
cargo install mdvs
mdvs init ./tasks
mdvs build ./tasks --set-model minishlab/potion-base-2M --force
mdvs --output json search "ready task" ./tasks \
  --mode hybrid \
  --where "status = 'ready' AND queue = 'agentboard-ready' AND agentboard.owner = 'Q'" \
  --limit 50 --no-build --no-update
```

Nested YAML is inferred as dotted leaf fields and remains queryable through the natural nested structure; array equality and `IN` predicates are rewritten to containment. Search JSON returns relative filenames, scores, line ranges, and matched chunks, but not the full frontmatter, so a generic command source should enrich each hit from the referenced Markdown file before applying shared mapping. [nested field model](https://github.com/edochi/mdvs/blob/v0.8.3/book/src/concepts/types.md#nested-objects-in-yaml) [array and nested queries](https://github.com/edochi/mdvs/blob/v0.8.3/book/src/search-guide.md#array-fields)

Operational costs are material: mdvs requires `mdvs.toml`, `.mdvs/`, and an embedding model even when queries later use BM25-only mode. The default model is about 480 MB; the smallest documented model is about 8 MB. The v0.8.3 Linux binary tested here was about 255 MB uncompressed. Builds abort on schema violations, `Array(Float)` filters are rejected, and arrays of objects are not first-class schema fields. [build pipeline](https://github.com/edochi/mdvs/blob/v0.8.3/book/src/commands/build.md) [search limitation](https://github.com/edochi/mdvs/blob/v0.8.3/book/src/search-guide.md#array-fields) [v0.8.3 release](https://github.com/edochi/mdvs/releases/tag/v0.8.3)

## Hyalo

### Verified capabilities

Hyalo explicitly targets folders of Markdown files with YAML frontmatter. `hyalo find` combines BM25 body search or regex with repeatable property filters. Supported property syntax includes `K=V`, `K!=V`, comparisons, existence/absence, and regex. Its JSON results contain a relative file path, modification time, parsed property map, optional title/tags/sections/tasks/links, body matches, and BM25 score. [README](https://github.com/ractive/hyalo/blob/v0.20.0/README.md) [`FindFilters`](https://github.com/ractive/hyalo/blob/v0.20.0/crates/hyalo-cli/src/cli/args.rs#L247-L329) [`FileObject`](https://github.com/ractive/hyalo/blob/v0.20.0/crates/hyalo-core/src/types.rs#L252-L281)

Hyalo separates metadata from lexical relevance: the BM25 implementation feeds only the body after frontmatter into scoring. This prevents `status`, `queue`, and other YAML values from accidentally influencing body relevance. [`find` implementation](https://github.com/ractive/hyalo/blob/v0.20.0/crates/hyalo-cli/src/commands/find/mod.rs)

It supports direct scans and an optional snapshot index. JSON is the default when stdout is piped, but AgentBoard should pass `--format json` explicitly. `find` is documented as read-only. [configuration and index docs](https://github.com/ractive/hyalo/blob/main/docs/configuration.md) [`find` command help source](https://github.com/ractive/hyalo/blob/v0.20.0/crates/hyalo-cli/src/cli/args.rs#L405-L477)

Install:

```bash
cargo install hyalo-cli
```

Example suitable for an AgentBoard adapter:

```bash
hyalo \
  --dir ./tasks \
  --format json \
  --quiet \
  find "architecture retry" \
  --property queue=agentboard-ready \
  --fields properties,title \
  --limit 50
```

The result envelope has `results[]`; each row has `file`, `properties`, and—when body search is used—`score`. AgentBoard can read `root/file` directly, reuse its own YAML parser, and avoid spawning one retrieval process per result.

### Limits and risks

- Property filters operate on top-level frontmatter keys. A local smoke test confirmed `status=ready` works while `agentboard.owner=Q` does not traverse the nested map. Nested fields remain present in the returned `properties` object and can still be mapped after selection.
- It is young. Crates.io shows `0.20.0`, first published in April 2026, and the project released `0.18.0`, `0.19.0`, and `0.20.0` across 18–19 July 2026. That activity is positive but signals interface churn. [crates.io API](https://crates.io/api/v1/crates/hyalo-cli) [releases](https://github.com/ractive/hyalo/releases)
- It provides ranked lexical search, not embeddings or semantic retrieval.
- The broad CLI includes mutation commands. AgentBoard must invoke only `find`; do not expose user-provided command fragments.

## VaultDB

### Verified capabilities

VaultDB treats folders of Markdown files as tables and parses YAML frontmatter into typed values. Its query DSL supports equality/inequality, numeric comparisons, `contains`, prefix/suffix checks, regex, existence/null checks, `&&`, `||`, `!`, repeatable `--where`, sorting, selection, and limits. Recursive descent is explicit with `--recursive`. [README query syntax](https://github.com/rusenbb/vaultdb/blob/v1.6.1/README.md#where-expression-syntax) [DSL parser](https://github.com/rusenbb/vaultdb/blob/v1.6.1/crates/vaultdb-core/src/dsl.rs) [query command](https://github.com/rusenbb/vaultdb/blob/v1.6.1/crates/vaultdb/src/commands/query.rs)

Body search is structurally separate from frontmatter. `_body` resolves to text after the closing frontmatter delimiter and works with `contains`, `matches`, `startswith`, and `endswith`. Raw content is loaded only when a body or graph predicate needs it. [body-search documentation](https://github.com/rusenbb/vaultdb/blob/v1.6.1/README.md#body-search) [`_body` implementation](https://github.com/rusenbb/vaultdb/blob/v1.6.1/crates/vaultdb-core/src/record.rs#L131-L143) [body-load detection](https://github.com/rusenbb/vaultdb/blob/v1.6.1/crates/vaultdb-core/src/filter.rs#L442-L469)

Install and query:

```bash
cargo install vaultdb --version 1.6.1

vaultdb query . \
  --vault ./tasks \
  --recursive \
  --where 'queue = agentboard-ready && _body contains "architecture retry"' \
  --select '_path,id,title,status,queue' \
  --limit 50 \
  --format json
```

JSON is a typed array. `_path` is vault-relative; selected arrays remain arrays and selected nested maps remain objects. The CLI scans current files directly—no daemon, cache, or index build is required. [output formats](https://github.com/rusenbb/vaultdb/blob/v1.6.1/README.md#query) [record model](https://github.com/rusenbb/vaultdb/blob/v1.6.1/crates/vaultdb-core/src/record.rs#L75-L90)

### Limits and risks

- Frontmatter predicates are top-level. Selecting `agentboard` returns the parsed nested object, but `agentboard.owner = Q` fails DSL parsing because `.` is not accepted in a field identifier.
- Body matching is deterministic substring or regex search, not relevance-ranked retrieval. `contains` is case-sensitive in the tested release.
- The CLI also exposes mutation commands. AgentBoard should invoke only `query` and construct arguments without a shell.
- It is young: `1.6.1` was published on 28 May 2026, though the release sequence from `1.1.0` through `1.6.1` shows active development. [crates.io API](https://crates.io/api/v1/crates/vaultdb) [repository tags](https://github.com/rusenbb/vaultdb/tags)

## mdq

### Verified capabilities

`mdq` recursively discovers Markdown, parses YAML frontmatter, and builds a fresh in-memory SQLite database for each invocation. The `files` table keeps `frontmatter_json` and body text separately; `props`, `tags`, `links`, and `sections` provide normalized secondary tables. Full SQLite SQL therefore supports nested frontmatter through `json_extract`, body predicates, joins, ordering, and `LIMIT` in one query. [README](https://github.com/davidgasquez/mdq/blob/5b62a20d1623268036e3e9d12d14955de81a2837/README.md) [recursive discovery](https://github.com/davidgasquez/mdq/blob/5b62a20d1623268036e3e9d12d14955de81a2837/mdq.py#L93-L113) [database schema](https://github.com/davidgasquez/mdq/blob/5b62a20d1623268036e3e9d12d14955de81a2837/mdq.py#L465-L531)

The CLI emits table, JSON, NDJSON, or CSV. It is stateless: no daemon or persistent index is required. [CLI options](https://github.com/davidgasquez/mdq/blob/5b62a20d1623268036e3e9d12d14955de81a2837/mdq.py#L62-L82) [output implementation](https://github.com/davidgasquez/mdq/blob/5b62a20d1623268036e3e9d12d14955de81a2837/mdq.py#L686-L712)

Install a pinned source revision and query:

```bash
uv tool install \
  "git+https://github.com/davidgasquez/mdq.git@5b62a20d1623268036e3e9d12d14955de81a2837"

mdq ./tasks --format ndjson \
  "SELECT
      path,
      json_extract(frontmatter_json, '$.id') AS id,
      json_extract(frontmatter_json, '$.title') AS title,
      json_extract(frontmatter_json, '$.status') AS status,
      json_extract(frontmatter_json, '$.agentboard.owner') AS owner
   FROM files
   WHERE json_extract(frontmatter_json, '$.queue') = 'agentboard-ready'
     AND body LIKE '%architecture%'
   ORDER BY path
   LIMIT 50"
```

A smoke test returned both a root note and a recursively discovered nested note, filtered through nested JSON paths, array membership via SQLite `json_each`, and body text, as bounded NDJSON rows.

### Limits and risks

- This falls outside the original npm/Cargo/Go/Pi ecosystem constraint. Installation is from Git, not PyPI.
- The package declares Python `>=3.13`. [package manifest](https://github.com/davidgasquez/mdq/blob/5b62a20d1623268036e3e9d12d14955de81a2837/pyproject.toml)
- Upstream has no tags or GitHub releases; the repository version is only `0.1.0`. Pin a commit or do not use it.
- Body matching is ordinary SQLite text matching, not BM25 or semantic ranking.
- The SQL interface is powerful but broad. AgentBoard should generate a fixed query shape from structured config instead of accepting arbitrary SQL if workspace configuration is not fully trusted.

## FMQL

### Verified capabilities

FMQL scans a workspace recursively, parses YAML frontmatter, and exposes each document as a packet with a stable workspace-relative id. Its Cypher subset supports equality, comparison, list membership/containment, regex, null/empty checks, AND/OR/NOT, ordering, and limits. Virtual `_path` and `_id` fields cannot be shadowed by frontmatter. [README](https://github.com/buyuk-dev/fmql/blob/core-v0.3.0/README.md) [workspace loader](https://github.com/buyuk-dev/fmql/blob/core-v0.3.0/packages/fmql/src/fmql/workspace.py) [query command](https://github.com/buyuk-dev/fmql/blob/core-v0.3.0/packages/fmql/src/fmql/cli/cmd_query.py)

The query command can add a search stage with `--search`. With the built-in grep backend, metadata selection and text search can run in one invocation. `RETURN t --format json` emits one JSON object per line shaped as `{id, frontmatter}`; the body is not emitted, so AgentBoard would read each selected file once. [grep backend](https://github.com/buyuk-dev/fmql/blob/core-v0.3.0/packages/fmql/src/fmql/search/backends/grep.py) [serialization](https://github.com/buyuk-dev/fmql/blob/core-v0.3.0/packages/fmql/src/fmql/serialization.py)

Install and query:

```bash
pipx install fmql==0.3.0

fmql query \
  'MATCH (t) WHERE t.queue = "agentboard-ready" RETURN t LIMIT 50' \
  --workspace ./tasks \
  --search 'architecture retry' \
  --format json \
  --limit 50
```

The official `fmql-semantic` plugin adds indexed BM25, dense embeddings, reciprocal-rank fusion, and optional reranking, but it is a second package with model/index dependencies. It was not needed for the qualifying smoke test and should not be assumed in a minimal AgentBoard backend. [plugin README](https://github.com/buyuk-dev/fmql/blob/semantic-v0.1.2/packages/fmql-semantic/README.md) [PyPI plugin](https://pypi.org/project/fmql-semantic/0.1.2/)

### Limits and risks

- Nested frontmatter predicates are unsupported in `0.3.0`; `t.agentboard.owner` fails parsing. Nested maps remain available in the returned `frontmatter` object for AgentBoard's own mapper.
- Built-in grep search checks body text first, then serialized frontmatter. It is not body-only and assigns every hit score `1.0`.
- `--search` requires `RETURN` to be one packet variable. Projected multi-column rows cannot be combined directly with search.
- The latest PyPI release is `0.3.0` from 3 May 2026. The repository has later May commits but no newer published package, so pin the tested version. [PyPI JSON API](https://pypi.org/pypi/fmql/json) [repository](https://github.com/buyuk-dev/fmql)

## Markbase

### Verified capabilities

Markbase recursively indexes Markdown into a derived DuckDB database before each query. `file.*` names file metadata, `note.*` names frontmatter, and bare fields are shorthand for frontmatter. Both expression mode and read-only `SELECT` support nested JSON paths, casts, SQL operators, ordering, grouping, and explicit `LIMIT`; `list_contains(...)` handles arrays. JSON is the default output and paths are root-relative. [README query docs](https://github.com/flyisland/markbase/blob/v0.9.5/README.md#query-notes) [query contract](https://github.com/flyisland/markbase/blob/v0.9.5/docs/design-docs/implemented/design-010-query-subsystem.md) [translator](https://github.com/flyisland/markbase/blob/v0.9.5/src/query/translator.rs#L334-L425)

Install and query:

```bash
cargo install markbase --version 0.9.5

markbase --base-dir ./tasks query \
  "SELECT file.path, note.id, note.title, note.status, note.agentboard.owner \
   FROM notes \
   WHERE note.queue = 'agentboard-ready' \
     AND list_contains(note.tags, 'urgent') \
   LIMIT 50"
```

The tested output was a JSON array keyed by selected names such as `file.path` and `note.agentboard.owner`. A default limit of `1000` is appended when the query omits one. [default limit](https://github.com/flyisland/markbase/blob/v0.9.5/src/query/executor.rs) [JSON output](https://github.com/flyisland/markbase/blob/v0.9.5/src/output.rs)

### Limits and risks

- The index schema stores file metadata, tags, links, embeds, and parsed frontmatter JSON, but not body text. A probe against `note.body` returned no matches because it is just an absent frontmatter field. [database schema](https://github.com/flyisland/markbase/blob/v0.9.5/src/db.rs#L42-L58) [indexing contract](https://github.com/flyisland/markbase/blob/v0.9.5/docs/design-docs/implemented/design-005-indexing.md)
- Selecting a whole nested object such as `note.agentboard` serializes it as a JSON-looking string; selecting a leaf such as `note.agentboard.owner` returns the scalar. AgentBoard should still reparse the source file.
- Every query refreshes a derived DuckDB index. This is headless and deterministic, but it adds sidecar state and more machinery than direct scanners.
- Crates.io latest is `0.9.5` from 26 March 2026, while GitHub has a `v0.9.6` release not published to crates.io. Pinning `0.9.5` avoids registry/repository ambiguity. [crates.io API](https://crates.io/api/v1/crates/markbase) [v0.9.6 release](https://github.com/flyisland/markbase/releases/tag/v0.9.6)

## mdbasequery

### Verified capabilities

`mdbasequery` scans a Markdown vault, parses YAML frontmatter with the `yaml` package, and exposes note properties plus file metadata—including raw file content—to an Obsidian Bases-style expression engine. It accepts query files, inline YAML, or repeatable CLI flags. [README](https://github.com/intellectronica/mdbasequery/blob/v0.0.1/README.md) [Markdown parser](https://github.com/intellectronica/mdbasequery/blob/v0.0.1/src/core/markdown.ts) [vault index](https://github.com/intellectronica/mdbasequery/blob/v0.0.1/src/core/vault-index.ts)

It emits JSON, JSONL, YAML, CSV, or Markdown. JSONL contains only projected fields, which makes an adapter simple and stable if AgentBoard always selects `file.path`, mapped frontmatter fields, and any raw properties it needs. [`serialize.ts`](https://github.com/intellectronica/mdbasequery/blob/v0.0.1/src/core/serialize.ts) [`types.ts`](https://github.com/intellectronica/mdbasequery/blob/v0.0.1/src/types.ts)

Install and query:

```bash
npm install -g mdbasequery

mdbasequery \
  --dir ./tasks \
  --filter 'queue == "agentboard-ready"' \
  --filter 'file.raw.contains("architecture")' \
  --select file.path \
  --select id \
  --select title \
  --select status \
  --select queue \
  --limit 50 \
  --format jsonl
```

A local smoke test against upstream `v0.0.1` returned the expected JSONL row. Nested expressions such as `agentboard.owner == "Q"` work, but strict mode records a row error when another document lacks `agentboard`; `--no-strict` avoids that failure. This makes optional nested fields more awkward than Hyalo's simple top-level filters.

### Limits and risks

- The package remains `0.0.1`; npm and GitHub show one release from 18 February 2026. [npm registry](https://registry.npmjs.org/mdbasequery/0.0.1) [release](https://github.com/intellectronica/mdbasequery/releases/tag/v0.0.1)
- Body search is expression-based substring matching over `file.raw`, not ranked retrieval. Frontmatter is part of `file.raw`, so an adapter must combine body predicates with explicit metadata predicates and accept that raw-text matches can occur in YAML.
- It scans at query time and does not provide QMD-style embeddings or reranking.
- Its compatibility target is Obsidian Bases. AgentBoard should pin a tested package version rather than follow `latest` blindly.

## md-fme

`md-fme` recursively scans Markdown and evaluates a frontmatter DSL with nested dot paths, string/list `contains`, comparisons, `exists`, `missing`, AND, and OR. It reads YAML and TOML frontmatter, although its write path prefers TOML. JSON output contains a count plus absolute paths and a rendered field map. [README](https://github.com/ai-tools-all/obsidian-fme/blob/b70cbf121e61647125c39b1886ebcc4191f81bc3/README.md) [query implementation](https://github.com/ai-tools-all/obsidian-fme/blob/b70cbf121e61647125c39b1886ebcc4191f81bc3/crates/md-fme/src/query.rs) [JSON renderer](https://github.com/ai-tools-all/obsidian-fme/blob/b70cbf121e61647125c39b1886ebcc4191f81bc3/crates/md-fme/src/render/json.rs)

```bash
cargo install md-fme --version 0.9.8

md-fme query \
  'status = ready AND agentboard.owner = Q' \
  --folder ./tasks \
  --depth 0 \
  --json \
  --verbose
```

`--depth 0` means unlimited recursion; the default is depth 3. A smoke test selected a nested YAML key correctly and returned arrays/maps as display strings inside `fields`. The blocker is result bounding: `query` has no limit option and no body predicate. AgentBoard could truncate parsed JSON after the process exits, but that does not bound upstream work or output. Version `0.9.8` was published 9 March 2026 under MIT. [CLI source](https://github.com/ai-tools-all/obsidian-fme/blob/b70cbf121e61647125c39b1886ebcc4191f81bc3/crates/md-fme/src/main.rs) [crates.io API](https://crates.io/api/v1/crates/md-fme)

## Vori

### Verified capabilities

Vori recursively scans Markdown, parses YAML with the `yaml` package, and returns JSON rows containing a vault-relative path, parsed frontmatter object, hashtags, wikilinks, and body. `query` supports repeatable AND filters, nested dot paths, scalar equality, and array membership. [README](https://github.com/Questi0nM4rk/vori/blob/faa0df88d592865a1b89b1abe509a0b797fbd27f/README.md) [query implementation](https://github.com/Questi0nM4rk/vori/blob/faa0df88d592865a1b89b1abe509a0b797fbd27f/src/lib/query.ts) [result type](https://github.com/Questi0nM4rk/vori/blob/faa0df88d592865a1b89b1abe509a0b797fbd27f/src/lib/types.ts)

Install and query:

```bash
npm install -g @questi0nm4rk/vori

vori query ./tasks \
  --tag status=ready \
  --tag queue=agentboard-ready \
  --tag agentboard.owner=Q \
  --json
```

A smoke test selected a recursively nested file using `status`, an array member (`tags=urgent`), and `agentboard.owner`; JSON preserved the nested object and full body. Its separate `search` command also found body text and returned the same complete row shape.

### Limits and risks

- `query` and `search` are separate code paths. Passing metadata flags to `search` does not apply them, so one invocation cannot combine body text with frontmatter predicates. [command dispatch](https://github.com/Questi0nM4rk/vori/blob/faa0df88d592865a1b89b1abe509a0b797fbd27f/src/main.ts#L182-L205) [search dispatch](https://github.com/Questi0nM4rk/vori/blob/faa0df88d592865a1b89b1abe509a0b797fbd27f/src/main.ts#L345-L356)
- There is no result limit, comparison operator, existence predicate, regex predicate, or sort control.
- The npm package exposes a `vori` binary, but the published executable starts with `#!/usr/bin/env bun` while its published npm metadata omits an `engines` declaration. Bun is therefore an undeclared runtime requirement. [npm registry metadata](https://registry.npmjs.org/@questi0nm4rk%2fvori/1.0.0) [published tarball](https://registry.npmjs.org/@questi0nm4rk/vori/-/vori-1.0.0.tgz)
- Unknown flags are silently skipped by the argument parser, which weakens configuration-error detection. [argument parser](https://github.com/Questi0nM4rk/vori/blob/faa0df88d592865a1b89b1abe509a0b797fbd27f/src/main.ts#L48-L139)
- npm `1.0.0` was published on 30 March 2026; the repository has later commits but no tags or GitHub releases. Pinning through npm is possible, but the source-to-package release trail is weak.

## vlt

### Verified capabilities

`vlt` is a headless Go CLI for Obsidian-style vaults. Search combines title/content text with property clauses such as `[status:active]`, and property-only queries are supported. It also offers regex search. [v0.11.0 README](https://github.com/paivot-ai/vlt/blob/v0.11.0/README.md#property-based-search)

Search supports JSON/YAML/CSV/TSV output. JSON search rows contain `title` and relative `path`, which is enough for AgentBoard to load and normalize the file itself. [`format.go`](https://github.com/paivot-ai/vlt/blob/v0.11.0/cmd/vlt/format.go#L195-L230)

Install:

```bash
go install github.com/paivot-ai/vlt/cmd/vlt@latest
```

Example:

```bash
VLT_VAULT_PATH=./tasks VLT_VAULT=tasks \
  vlt search 'query=architecture [queue:agentboard-ready]' --json
```

### Limits and risks

- The project explicitly uses a simple string-based frontmatter parser rather than a full YAML parser. It supports common key/value and list forms, but complex YAML can diverge from AgentBoard's `yaml_serde` parsing. [parsing scope](https://github.com/paivot-ai/vlt/blob/v0.11.0/README.md#important-parsing-scope)
- Property filters are top-level only. [v0.11.0 notes](https://github.com/paivot-ai/vlt/blob/v0.11.0/README.md#whats-new-in-v0110)
- The current source requires Go 1.26+. [installation](https://github.com/paivot-ai/vlt/blob/v0.11.0/README.md#installation)
- Search output omits the matched frontmatter, requiring AgentBoard to read every selected file.
- It is vault-oriented and resolves Obsidian configuration unless `VLT_VAULT_PATH` is supplied. That is unnecessary coupling for plain task directories.

Maintenance is credible but still young: `v0.11.0` was released 9 June 2026, after `v0.10.1` and `v0.10.2` in May. [releases](https://github.com/paivot-ai/vlt/releases)

## Krafna

Krafna recursively walks Markdown files, parses YAML frontmatter with `gray_matter`, and exposes it through an SQL-like `FRONTMATTER_DATA(path)` source. It supports nested fields, list membership with `IN`, comparisons, boolean expressions, ordering, JSON output, and absolute `file.path` values. [README](https://github.com/7sedam7/krafna/blob/v0.5.6/README.md) [Markdown loader](https://github.com/7sedam7/krafna/blob/v0.5.6/src/libs/data_fetcher/markdown_fetcher.rs) [serializer](https://github.com/7sedam7/krafna/blob/v0.5.6/src/libs/serializer.rs)

```bash
cargo install krafna --version 0.5.6

krafna \
  "SELECT file.path, id, title, status \
   FROM FRONTMATTER_DATA('./tasks') \
   WHERE status == 'ready' AND 'urgent' IN tags" \
  --json
```

The official README explicitly says `LIMIT` is unsupported. Worse, a smoke test that appended `LIMIT 1` exited successfully and returned both matching rows, so an adapter cannot trust the query text to bound output. Krafna also has no body search. The current crates.io release is still `0.5.6` from 1 March 2025, despite later repository commits. This disqualifies it from the default backend contract. [unsupported clauses](https://github.com/7sedam7/krafna/blob/v0.5.6/README.md#other) [crates.io API](https://crates.io/api/v1/crates/krafna)

## fmd

`fmd` parses YAML frontmatter and filters top-level custom fields using `--field field:pattern`. It emits paths, including safe NUL-delimited output. It needs no index. [README](https://github.com/zhouer/fmd/blob/v0.1.1/README.md) [crates.io API](https://crates.io/api/v1/crates/fmd)

Install and select:

```bash
cargo install fmd
fmd --field 'queue:agentboard-ready'
```

The problem is body search. Its `--full-text` option expands tag detection to the whole file; it does not accept an arbitrary lexical body query. The official examples pipe selected paths to `grep`, making the real backend a two-command `fmd | grep` composition. [full-text documentation](https://github.com/zhouer/fmd/blob/v0.1.1/README.md#full-text-search) [Unix composition](https://github.com/zhouer/fmd/blob/v0.1.1/README.md#usage-with-unix-tools)

This is acceptable only if the proposed source is explicitly a metadata-only Markdown source. It is not an honest QMD search alternative by itself.

## Rejected or secondary candidates

### dotmd-cli (npm)

`dotmd-cli` is actively released and supports JSON queries plus optional body scanning. However, it requires `status`, has type-specific lifecycle semantics, and exposes a fixed set of query fields (`type`, `status`, `owner`, `surface`, `module`, `domain`, and related workflow concepts). AgentBoard would inherit another tool's plan/ADR/RFC ontology instead of querying arbitrary Markdown metadata. [README](https://github.com/reowens/dotmd/blob/v0.70.3/README.md) [`query.mjs`](https://github.com/reowens/dotmd/blob/v0.70.3/src/query.mjs) [npm registry](https://registry.npmjs.org/dotmd-cli/0.70.3)

### zk (Go)

`zk` is mature and offers indexed FTS over titles/bodies plus tag filtering. It indexes arbitrary frontmatter and can print metadata through templates, but its metadata-aware filters are specialized around tags, dates, aliases, links, and note attributes. There is no documented arbitrary `status=ready` or `queue=...` frontmatter predicate. [frontmatter docs](https://zk-org.github.io/zk/notes/note-frontmatter.html) [filtering docs](https://zk-org.github.io/zk/notes/note-filtering.html) [v0.15.5 release](https://github.com/zk-org/zk/releases/tag/v0.15.5)

### mdvault (Cargo)

`mdvault` provides indexed search and enforces frontmatter schemas, but it is intentionally opinionated around note types such as task/project/meeting/zettel. Its documented list filters are type/date oriented rather than arbitrary property predicates. It would impose a second task model on AgentBoard. [repository](https://github.com/agustinvalencia/mdvault) [crates.io API](https://crates.io/api/v1/crates/mdvault)

### matterof (Cargo)

`matterof 0.2.1` is a real structural YAML-frontmatter tool and supports RFC 9535 JSONPath, nested fields, arrays, directory recursion, and JSON/YAML output. It is an extractor/editor, not a vault selector: JSONPath runs within each document's frontmatter and emits matched values, while the CLI has no document-level predicate that returns only matching file rows plus mapped fields. A smoke test over a directory returned nested values for every file, while attempted root predicates produced no selected paths. It also has no body search or result limit. [README](https://github.com/cdfmlr/matterof/blob/fb82ac328f1b7433a34db0b39da4c2176a79d55b/README.md) [crates.io API](https://crates.io/api/v1/crates/matterof)

### markedup (Go)

`markedup` has recursive keyword, semantic, and reranked body search plus JSON output, but it unmarshals YAML into a fixed graph schema (`id`, `title`, `entity-type`, `confidence`, tags, relationships, and temporal metadata). Search has no arbitrary frontmatter predicate; export filters only entity type and tag. AgentBoard task fields such as arbitrary `status`, `queue`, or nested mappings therefore do not satisfy the mandatory selector contract. Install is `go install github.com/Clarit-AI/markedup/cmd/markedup@latest`. [CLI reference](https://github.com/Clarit-AI/markedup/blob/0c5745b5a98610e01f4d358fee089a90aeafd6a2/docs/cli-reference.md) [schema reference](https://github.com/Clarit-AI/markedup/blob/0c5745b5a98610e01f4d358fee089a90aeafd6a2/docs/schema-reference.md) [Go module](https://github.com/Clarit-AI/markedup/blob/0c5745b5a98610e01f4d358fee089a90aeafd6a2/go.mod)

### Flatmark

The current `sake92/flatmark` project is a Scala static-site generator. Its public contract is Markdown-to-website generation, not metadata querying, and there is no registry CLI matching the required frontmatter-selector behavior. The npm and crates names `flatmark` are absent. [repository](https://github.com/sake92/flatmark) [README](https://github.com/sake92/flatmark/blob/a1a1d6cc59e002ae0adfdaff2a4487be184e2bb4/README.md)

### MDQL (Cargo)

`mdql 0.5.37` offers full SQL over frontmatter columns and H2 content columns with JSON output, but it is a schema-first Markdown database rather than an arbitrary-vault scanner. Existing files must conform to MDQL table/schema configuration before queries are reliable. That is a larger migration than an AgentBoard source adapter, and the package is AGPL-3.0-only. [README](https://github.com/mdql-db/mdql/blob/v0.5.37/README.md) [CLI manifest](https://github.com/mdql-db/mdql/blob/v0.5.37/crates/mdql/Cargo.toml) [crates.io API](https://crates.io/api/v1/crates/mdql)

### `fmql` name collision

The Cargo crate `fmql 0.3.0` is an unrelated general file manager. The qualifying frontmatter query engine is the Python package from `buyuk-dev/fmql`; `cargo install fmql` installs the wrong tool. [Cargo crate API](https://crates.io/api/v1/crates/fmql) [Python FMQL repository](https://github.com/buyuk-dev/fmql)

### Additional screened candidates

- **`fmq 0.0.2` (Cargo)** parses nested frontmatter paths, but requires every input file as a positional argument, supports only one string-valued condition, emits ad-hoc text rather than JSON, has no recursion or limit, and has not been updated since October 2023. It is not an adapter backend. [README](https://github.com/thales-maciel/fmq/blob/v0.0.2/README.md) [CLI implementation](https://github.com/thales-maciel/fmq/blob/v0.0.2/src/main.rs) [crates.io API](https://crates.io/api/v1/crates/fmq)
- **`md-db-rs`** has strong frontmatter list filters and JSON projection, plus a separate full-text search command. It is not published to crates.io, has no tags/releases, is AGPL-3.0-or-later, and its list command has no limit. Installation currently requires a Git/source Cargo install rather than a registry package. [repository](https://github.com/decisiongraph/md-db-rs/tree/41b4eaefa76039f5aae274a37da998a7065a7ade) [list command](https://github.com/decisiongraph/md-db-rs/blob/41b4eaefa76039f5aae274a37da998a7065a7ade/crates/md-db-cli/src/commands/list.rs) [CLI manifest](https://github.com/decisiongraph/md-db-rs/blob/41b4eaefa76039f5aae274a37da998a7065a7ade/crates/md-db-cli/Cargo.toml)
- **`yamatter`** inspects or transforms parsed frontmatter but does not provide document-level predicates without user-authored JavaScript. They are extraction tools, not search backends. [Yamatter README](https://github.com/danburzo/yamatter)
- **`grubber`** is a strong flat-record extractor with recursive scanning, frontmatter filters, JSON/JSONL, and array matching. Its Go module path is simply `grubber`, so it cannot be installed through a stable `go install github.com/...@version` path; it also has no result limit, no body predicate, and deliberately degrades nested mappings. [README](https://github.com/rhsev/grubber) [`go.mod`](https://github.com/rhsev/grubber/blob/main/go.mod)

### Pi packages

Pi packages bundle extensions, skills, prompts, and themes; installation does not imply that the package exposes a process-level CLI. [Pi package docs](https://github.com/badlogic/pi-mono/blob/main/packages/coding-agent/docs/packages.md)

- `pi-knowledge` is an extension and has no package `bin`. Its implemented filters are `file_type` and `path_pattern`; the code comment calls these metadata filters, but they are not YAML-frontmatter filters. [`package.json`](https://github.com/nczz/pi-knowledge/blob/v0.5.2/package.json) [`SearchOptions`](https://github.com/nczz/pi-knowledge/blob/v0.5.2/src/engine.ts#L86-L100) [filter implementation](https://github.com/nczz/pi-knowledge/blob/v0.5.2/src/engine.ts#L1573-L1585)
- `pi-memctx` exposes Pi memory tools, but semantic/deep retrieval resolves QMD and keyword fallback is grep-style Markdown search. It does not solve the frontmatter predicate requirement. [search docs](https://github.com/weauratech/pi-memctx/blob/0dada27a2ade0b2216db6f688495712e22a1e0fc/docs/search.md)
- `pi-knowledge-search`, `pi-memory`, and `pi-memory-md` are Pi extensions without standalone package binaries in their npm manifests. They are agent tools, not stable subprocess APIs for a Rust adapter. [pi-knowledge-search registry metadata](https://registry.npmjs.org/pi-knowledge-search/latest) [pi-memory registry metadata](https://registry.npmjs.org/pi-memory/latest) [pi-memory-md registry metadata](https://registry.npmjs.org/pi-memory-md/latest)

**Conclusion for Pi:** no verified fit. Do not shell out to a whole Pi agent session just to query task files.

## Recommended AgentBoard integration

[bias: prefer a narrow adapter over a generic abstraction before two backends prove they share a stable contract]

Add a separate `agentboard-source-hyalo` source. Do not rename QMD or create a generic `markdown` source yet; QMD owns semantic query semantics while Hyalo owns BM25/property-filter semantics.

Suggested configuration shape:

```toml
[[sources]]
id = "local-ready"

[sources.source]
kind = "hyalo"
root = "tasks"
query = "architecture retry" # optional; omit for metadata-only selection
properties = ["queue=agentboard-ready"]
limit = 50

[sources.source.map]
id = "id"
title = "title"
status = "status"
url = "url"
```

Suggested execution flow:

```text
Hyalo source config
  -> validate root/query/property filters/limit
  -> hyalo --dir ROOT --format json --quiet find ...
  -> parse results[]
  -> canonicalize ROOT/result.file and reject escape outside ROOT
  -> read Markdown once
  -> reuse shared YAML frontmatter parser
  -> map id/title/status/url, including nested dot paths
  -> item.id = stable root-relative document path
  -> raw = { hyalo: row, frontmatter, body }
```

Recommended command construction:

```text
hyalo --dir <root> --format json --quiet find [query]
  --property <filter>...
  --fields properties,title
  --limit <limit>
```

Implementation constraints:

1. Pass every argument through `std::process::Command`; never build a shell command string.
2. Treat Hyalo's root-relative `file` as untrusted. Canonicalize and ensure it remains under `root`.
3. Reparse the selected file with AgentBoard's YAML parser. The CLI's property map is selection evidence, not the normalization authority.
4. Keep `item.id` path-based, matching the current QMD document-reference identity rule; keep frontmatter `id` as `reference_id`.
5. Preserve the Hyalo row, parsed frontmatter, and body in `raw`.
6. Check `hyalo` in `doctor`, and include stderr when exit status is non-zero.
7. Pin and test a minimum Hyalo version because the upstream CLI is moving quickly.
8. Add contract fixtures for top-level property filters, body BM25 search, metadata-only selection, nested mapped fields, missing required fields, duplicate identities, invalid paths, empty results, and malformed JSON.

## Local verification performed

The following checks used upstream source clones and temporary fixtures containing `status`, `queue`, nested `agentboard.owner`, arrays, numeric fields, recursive subdirectories, body text, and one file without frontmatter:

- Temporary install roots successfully installed `vaultdb 1.6.1`, `markbase 0.9.5`, `md-fme 0.9.8`, `krafna 0.5.6`, and `matterof 0.2.1` with Cargo. A temporary `pipx` home successfully installed `fmql==0.3.0` and reported version `0.3.0`.
- Hyalo upstream commit `c42fa6ff1793`: `find 'architecture retry' --property status=ready --fields properties,title` returned one JSON result with `TASK-001.md`, the complete parsed property map, and a BM25 score.
- Hyalo nested filter check: `--property agentboard.owner=Q` returned no results, confirming property filters do not traverse nested maps.
- VaultDB `1.6.1`: `query . --recursive --where 'status = ready' --where 'tags contains urgent' --select '_path,id,title,status,queue,tags,agentboard' --limit 10 --format json` returned two typed rows, including the recursive file and nested `agentboard` objects.
- VaultDB `1.6.1`: `--where 'status = ready && _body contains "Architecture retry"'` returned only `TASK-001.md`; `_body` excluded YAML frontmatter. `agentboard.owner = Q` failed parsing, confirming top-level-only predicates.
- mdq commit `5b62a20d1623`: one SQL query combined nested `$.agentboard.owner`, recursive paths, array membership through `json_each`, body text, ordering, and `LIMIT 50`; NDJSON returned the expected root and nested notes.
- FMQL `0.3.0`: a Cypher query over `status` plus `tags CONTAINS "urgent"` returned both recursive ready files as newline-delimited JSON rows with relative `_path` values.
- FMQL `0.3.0`: combining `--search 'architecture retry'` with `WHERE status = "ready" RETURN t` returned the expected packet; searching `agentboard-ready` also matched serialized frontmatter. `t.agentboard.owner` failed parsing.
- Markbase `0.9.5`: automatic indexing plus `agentboard.owner == 'Q' LIMIT 10` selected the nested field correctly. Explicit `SELECT file.path, note.id, note.agentboard.owner ...` returned JSON with relative paths.
- Markbase `0.9.5`: `list_contains(tags, 'urgent')` worked. A `note.body LIKE '%retry%'` probe returned no rows, consistent with the body-free index schema.
- mdbasequery `v0.0.1`: `status == "ready"` plus selected fields returned one JSONL row.
- mdbasequery `v0.0.1`: `file.raw.contains("retry")` plus `status == "ready"` returned the expected row.
- mdbasequery nested expression check: `agentboard.owner == "Q"` worked with `--no-strict`; strict mode exited `1` because the second document lacked `agentboard`.
- md-fme `0.9.8`: `query 'agentboard.owner = Q' --depth 0 --json --verbose` selected the nested field and emitted absolute paths; the CLI exposed no limit flag.
- Vori source commit `faa0df88d592`: `query --tag status=ready --tag tags=urgent --tag agentboard.owner=Q --json` selected a recursively nested note and preserved its parsed frontmatter and body. Its separate `search retry --json` found body text but could not combine metadata filters.
- Krafna `0.5.6`: nested `agentboard.owner` and list membership worked with JSON output. Appending unsupported `LIMIT 1` exited `0` and returned both rows.
- matterof `0.2.1`: recursive `get --query 'agentboard.owner' --format json` emitted values for every frontmatter file; root predicate attempts emitted no selected file rows.
- fmd upstream `v0.1.1`: `-f status:ready` emitted `./TASK-001.md`.
- vlt source could not be executed in the local Go 1.18 environment because upstream requires Go 1.26; its documented CLI and output contract were verified from tagged source and tests instead.

## Unresolved gaps

- No comparative performance benchmark was run across Hyalo, VaultDB, mdq, FMQL, Markbase, mdbasequery, and Vori on a large real vault.
- mdq has no registry release, tag, or large-vault benchmark; the smoke test used its pinned source commit through `uv run` with PyYAML.
- FMQL's optional semantic plugin was verified from first-party source and registry metadata but not installed or smoke-tested; it adds model and index dependencies outside the minimal CLI contract.
- Markbase `v0.9.6` exists as a GitHub release but was not published to crates.io; the tested installable registry version is `0.9.5`.
- `markedup` has no tagged release or package registry version to pin, and it fails the metadata predicate requirement before runtime testing matters.
- vlt remains source-verified rather than locally executed because the available Go toolchain is older than upstream's requirement.

## Source index

### AgentBoard

- [`agentboard-source-qmd` implementation](../../../pkgs/crates/agentboard-source-qmd/src/lib.rs)
- [QMD source documentation](../../../pkgs/crates/agentboard-source-qmd/src/docs.md)

### Hyalo

- [Repository README, tag `v0.20.0`](https://github.com/ractive/hyalo/blob/v0.20.0/README.md)
- [`find` CLI arguments](https://github.com/ractive/hyalo/blob/v0.20.0/crates/hyalo-cli/src/cli/args.rs#L247-L329)
- [`find` implementation](https://github.com/ractive/hyalo/blob/v0.20.0/crates/hyalo-cli/src/commands/find/mod.rs)
- [JSON result type](https://github.com/ractive/hyalo/blob/v0.20.0/crates/hyalo-core/src/types.rs#L252-L281)
- [crates.io API](https://crates.io/api/v1/crates/hyalo-cli)
- [GitHub releases](https://github.com/ractive/hyalo/releases)

### VaultDB

- [Repository README, tag `v1.6.1`](https://github.com/rusenbb/vaultdb/blob/v1.6.1/README.md)
- [Query CLI implementation](https://github.com/rusenbb/vaultdb/blob/v1.6.1/crates/vaultdb/src/commands/query.rs)
- [Where-expression parser](https://github.com/rusenbb/vaultdb/blob/v1.6.1/crates/vaultdb-core/src/dsl.rs)
- [Record and body virtual fields](https://github.com/rusenbb/vaultdb/blob/v1.6.1/crates/vaultdb-core/src/record.rs)
- [crates.io API](https://crates.io/api/v1/crates/vaultdb)

### mdq

- [Repository README, commit `5b62a20`](https://github.com/davidgasquez/mdq/blob/5b62a20d1623268036e3e9d12d14955de81a2837/README.md)
- [Package manifest](https://github.com/davidgasquez/mdq/blob/5b62a20d1623268036e3e9d12d14955de81a2837/pyproject.toml)
- [Recursive scanner and Markdown parser](https://github.com/davidgasquez/mdq/blob/5b62a20d1623268036e3e9d12d14955de81a2837/mdq.py#L93-L168)
- [SQLite schema and loader](https://github.com/davidgasquez/mdq/blob/5b62a20d1623268036e3e9d12d14955de81a2837/mdq.py#L465-L650)
- [Structured output implementation](https://github.com/davidgasquez/mdq/blob/5b62a20d1623268036e3e9d12d14955de81a2837/mdq.py#L686-L712)

### FMQL

- [Repository README, tag `core-v0.3.0`](https://github.com/buyuk-dev/fmql/blob/core-v0.3.0/README.md)
- [Query CLI](https://github.com/buyuk-dev/fmql/blob/core-v0.3.0/packages/fmql/src/fmql/cli/cmd_query.py)
- [Recursive workspace loader](https://github.com/buyuk-dev/fmql/blob/core-v0.3.0/packages/fmql/src/fmql/workspace.py)
- [Built-in grep backend](https://github.com/buyuk-dev/fmql/blob/core-v0.3.0/packages/fmql/src/fmql/search/backends/grep.py)
- [PyPI `fmql 0.3.0`](https://pypi.org/project/fmql/0.3.0/)
- [PyPI `fmql-semantic 0.1.2`](https://pypi.org/project/fmql-semantic/0.1.2/)

### Markbase

- [Repository README, tag `v0.9.5`](https://github.com/flyisland/markbase/blob/v0.9.5/README.md)
- [Query subsystem contract](https://github.com/flyisland/markbase/blob/v0.9.5/docs/design-docs/implemented/design-010-query-subsystem.md)
- [Nested/list field translator](https://github.com/flyisland/markbase/blob/v0.9.5/src/query/translator.rs)
- [Index database schema](https://github.com/flyisland/markbase/blob/v0.9.5/src/db.rs)
- [crates.io API](https://crates.io/api/v1/crates/markbase)

### md-fme

- [Repository README, commit `b70cbf1`](https://github.com/ai-tools-all/obsidian-fme/blob/b70cbf121e61647125c39b1886ebcc4191f81bc3/README.md)
- [Query implementation](https://github.com/ai-tools-all/obsidian-fme/blob/b70cbf121e61647125c39b1886ebcc4191f81bc3/crates/md-fme/src/query.rs)
- [JSON renderer](https://github.com/ai-tools-all/obsidian-fme/blob/b70cbf121e61647125c39b1886ebcc4191f81bc3/crates/md-fme/src/render/json.rs)
- [crates.io API](https://crates.io/api/v1/crates/md-fme)

### Krafna

- [Repository README, tag `v0.5.6`](https://github.com/7sedam7/krafna/blob/v0.5.6/README.md)
- [Recursive Markdown loader](https://github.com/7sedam7/krafna/blob/v0.5.6/src/libs/data_fetcher/markdown_fetcher.rs)
- [crates.io API](https://crates.io/api/v1/crates/krafna)

### mdbasequery

- [Repository README, tag `v0.0.1`](https://github.com/intellectronica/mdbasequery/blob/v0.0.1/README.md)
- [Markdown/frontmatter parser](https://github.com/intellectronica/mdbasequery/blob/v0.0.1/src/core/markdown.ts)
- [Vault index](https://github.com/intellectronica/mdbasequery/blob/v0.0.1/src/core/vault-index.ts)
- [Output serializer](https://github.com/intellectronica/mdbasequery/blob/v0.0.1/src/core/serialize.ts)
- [npm registry](https://registry.npmjs.org/mdbasequery/0.0.1)

### Vori

- [Repository README, commit `faa0df8`](https://github.com/Questi0nM4rk/vori/blob/faa0df88d592865a1b89b1abe509a0b797fbd27f/README.md)
- [CLI dispatch and argument parser](https://github.com/Questi0nM4rk/vori/blob/faa0df88d592865a1b89b1abe509a0b797fbd27f/src/main.ts)
- [Nested frontmatter query implementation](https://github.com/Questi0nM4rk/vori/blob/faa0df88d592865a1b89b1abe509a0b797fbd27f/src/lib/query.ts)
- [JSON row type](https://github.com/Questi0nM4rk/vori/blob/faa0df88d592865a1b89b1abe509a0b797fbd27f/src/lib/types.ts)
- [npm registry metadata](https://registry.npmjs.org/@questi0nm4rk%2fvori/1.0.0)

### vlt

- [Repository README, tag `v0.11.0`](https://github.com/paivot-ai/vlt/blob/v0.11.0/README.md)
- [Search output formatter](https://github.com/paivot-ai/vlt/blob/v0.11.0/cmd/vlt/format.go#L195-L230)
- [Go package](https://pkg.go.dev/github.com/paivot-ai/vlt@v0.11.0)
- [GitHub release](https://github.com/paivot-ai/vlt/releases/tag/v0.11.0)

### Other candidates and Pi

- [fmd `v0.1.1` README](https://github.com/zhouer/fmd/blob/v0.1.1/README.md)
- [dotmd `v0.70.3` README](https://github.com/reowens/dotmd/blob/v0.70.3/README.md)
- [zk frontmatter docs](https://zk-org.github.io/zk/notes/note-frontmatter.html)
- [zk filtering docs](https://zk-org.github.io/zk/notes/note-filtering.html)
- [mdvault repository](https://github.com/agustinvalencia/mdvault)
- [matterof repository](https://github.com/cdfmlr/matterof)
- [markedup CLI reference](https://github.com/Clarit-AI/markedup/blob/0c5745b5a98610e01f4d358fee089a90aeafd6a2/docs/cli-reference.md)
- [Flatmark repository](https://github.com/sake92/flatmark)
- [MDQL `v0.5.37` README](https://github.com/mdql-db/mdql/blob/v0.5.37/README.md)
- [`fmq 0.0.2` source](https://github.com/thales-maciel/fmq/blob/v0.0.2/src/main.rs)
- [`md-db-rs` list command](https://github.com/decisiongraph/md-db-rs/blob/41b4eaefa76039f5aae274a37da998a7065a7ade/crates/md-db-cli/src/commands/list.rs)
- [Yamatter repository](https://github.com/danburzo/yamatter)
- [grubber repository](https://github.com/rhsev/grubber)
- [Pi package documentation](https://github.com/badlogic/pi-mono/blob/main/packages/coding-agent/docs/packages.md)
- [`pi-knowledge` package manifest](https://github.com/nczz/pi-knowledge/blob/v0.5.2/package.json)
- [`pi-knowledge` search implementation](https://github.com/nczz/pi-knowledge/blob/v0.5.2/src/engine.ts#L1456-L1585)
- [`pi-memctx` search documentation](https://github.com/weauratech/pi-memctx/blob/0dada27a2ade0b2216db6f688495712e22a1e0fc/docs/search.md)
