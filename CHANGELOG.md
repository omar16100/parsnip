# Changelog

All notable changes to this project are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this
project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html). All five
crates (`parsnip-core`, `parsnip-storage`, `parsnip-search`, `parsnip-mcp`, `parsnip-cli`)
share one version.

## [Unreleased]

## [0.2.0] - 2026-09-27

Install with `cargo install parsnip-cli` (the binary is `parsnip`; the crates.io crate
named `parsnip` is an unrelated project) or from the binaries on the GitHub release.

### Added

- Remote mode. The ReDB backend lets only one process open the database, so a running
  `parsnip serve` used to lock every other `parsnip` command out. Now one
  `parsnip serve --transport sse` daemon owns the database and other CLI and MCP processes
  reach it over HTTP through a JSON-RPC storage API (`storage/*` and `search/query`).
  - Clients pick the daemon with `--server`, `PARSNIP_SERVER` or the `server_url` config
    key, in that order of precedence. `--local` uses the local database instead.
  - `parsnip serve` (stdio) with `PARSNIP_SERVER` set runs as an MCP proxy that never
    opens the database.
  - `GET /health` reports the version, capabilities and maximum request size, and clients
    check it when they connect.
  - New `parsnip-cli` features: `remote` (on by default) and `remote-tls` (rustls, for
    `https://` daemon URLs). `sse` is now on by default as well.
- HTTP/SSE server authentication: a bearer token from `--auth-token` or
  `PARSNIP_AUTH_TOKEN`, and binding a non-loopback address requires `--allow-remote` plus
  a token.
- Input checks on MCP writes: `create_entities` checks the batch size and the project name,
  entity name, observation and tag lengths, and `create_relations` checks the batch size
  and entity name lengths (limits in `crates/parsnip-core/src/limits.rs`).
  `traverse_graph` caps depth at 50.
- Library API: `Relation` has optional `from_project_id` and `to_project_id` fields for
  relations between projects, and `StorageBackend` has `get_all_relations_all_projects`,
  `get_relations_for_entity_global`, `get_or_create_project`, `save_entities_batch` and
  `save_relations_batch`.

### Changed

- `--format json` and `--format csv` now take effect for `entity`, `relation`, `project`
  and `search` (they were parsed and ignored). JSON works for all of their subcommands
  except `project use`, which still prints text. CSV works for `entity list`,
  `relation list`, `project list` and `search`; the other subcommands in those groups exit
  with status 2 before doing anything. `--format` is one global option, and `export` reads
  it (`json`, `csv` or `graphml`).
- Breaking: short options that collided with global ones are gone. Use `--force` for
  `entity delete` and `project delete`, `--description` for `project create`, `--depth` for
  `relation traverse` and `--target-project` for `import`; `-f`, `-d` and `-p` now always
  mean `--format`, `--data-dir` and `--project`.
- Breaking for library users: `StorageBackend` implementations must provide
  `get_all_relations_all_projects` and `get_relations_for_entity_global` (the other new
  methods have default implementations), and `Relation` struct literals need the two new
  fields.
- `project stats` breakdowns and `relation traverse` entity lists print in a stable,
  sorted order.
- Full-text search builds its Tantivy index in memory per query instead of keeping one
  under `index/` in the data directory.
- `parsnip import` (including `--from-knowledgegraph`) writes entities and relations in
  batches.
- CLI commands run on a single-threaded Tokio runtime; `serve` keeps a multi-threaded one.
- Logs go to stderr.
- HTTP/SSE server: CORS allows only `localhost` and `127.0.0.1` origins on ports 3000 and
  8080 (it allowed any origin), and request bodies are limited to 32 MiB.
- The data directory is created with mode 0700 on Unix.
- CSV output prefixes cells that start with `=`, `+`, `-` or `@` with `'`, so spreadsheets
  do not evaluate them as formulas.
- Minimum supported Rust version is 1.88 (was 1.75).
- Each crate's package now includes the README and both license files, plus keywords,
  categories and a homepage link.
- Releases include a `checksums.sha256` file, which `install.sh` checks.
- Documentation: install instructions name the `parsnip-cli` crate, a Remote Mode section
  was added, performance figures are labelled as design targets, and the README and site
  no longer list a `vector` search mode or feature, which was never implemented.

### Fixed

- `parsnip completions <shell>` panicked for every shell (conflicting short options).
- `parsnip export` panicked while parsing its arguments (two `--format` options shared one
  argument id).
- Two clients creating the same project at the same moment could end up with two project
  ids for one name, orphaning entities. Project creation is now atomic for the CLI, MCP
  tools, MCP proxies, `project create` and `import`, including SQLite across processes.
- A zero page size in a search request could abort the daemon. Page sizes are now clamped
  to 1..=1000.
- The MCP stdio server created a new buffered reader for every request; it now keeps one
  for the whole session.
- CLI search fetched relations once per matching entity; it now reads them once per
  project. Traversal looks up neighbours through an adjacency map.

### Security

- Dependency updates for published advisories: lz4_flex 0.11.6 (GHSA-vvp9-7p8x-rfvv),
  oneshot 0.1.13 (GHSA-rvr2-r3pv-5m4p), time 0.3.55 (GHSA-r6v5-fh4h-64xc), bytes 1.12.1
  (GHSA-434x-w66g-qw3r), rand 0.8.8 and 0.9.5 (GHSA-cq8v-f236-94qc). These pins come from
  `Cargo.lock`, so they apply to the release binaries and to `cargo install --locked`.
- Daemon hardening: the token is compared in constant time, an empty token is refused,
  and its value is not shown in `--help`. A daemon without a token rejects requests whose
  `Host` header names something other than a loopback address (`/health` excepted), which
  blocks DNS rebinding from web pages. The bind address is resolved and checked before
  the database opens. Clients do not follow redirects. Storage and search RPC responses
  are not broadcast to SSE subscribers.

### Known limitations

- Concurrent updates to one entity from different clients are last-write-wins.
- Deleting a project while another client writes to it can leave that client's new
  entities under the deleted project.
- On the HTTP/SSE transport, MCP responses are broadcast to every authenticated `/sse`
  subscriber.
- The `create_relations` MCP tool schema lists `fromProjectId` and `toProjectId`, but the
  server ignores them: relations are created in the `projectId` project.
- `add_observations` and `add_tags` do not apply the length checks that
  `create_entities` does.
- In remote mode, `relation traverse`, `relation find-path` and
  `search --include-relations` fetch whole projects to the client.
- The `lru` advisory (GHSA-rhfx-m35p-ff5j) reaches the build through tantivy 0.22 and needs
  tantivy 0.26 to clear. Tantivy keeps its cache private and does not call the affected
  `IterMut`.

## [0.1.0] - 2025-12-14

First release: the five crates on crates.io and binaries for Linux x86_64 and macOS
(x86_64, arm64).

### Added

- Knowledge graph of projects, entities (with observations and tags) and typed relations.
- ReDB (default) and SQLite storage backends, and an in-memory backend for tests.
- Exact, fuzzy and full-text (Tantivy BM25) search, within one project or across all.
- Graph traversal: breadth-first, Dijkstra shortest path, filters by entity and relation
  type.
- `parsnip` CLI with `entity`, `relation`, `search`, `project`, `import` (JSON, or
  `--from-knowledgegraph` for a knowledgegraph-mcp SQLite database), `export`, `serve`,
  `config` and `completions` commands.
- MCP server with 13 tools over stdio, and an HTTP/SSE transport behind the `sse` feature.
- `install.sh` shell installer for the release binaries.

[Unreleased]: https://github.com/omar16100/parsnip/compare/v0.2.0...HEAD
[0.2.0]: https://github.com/omar16100/parsnip/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/omar16100/parsnip/releases/tag/v0.1.0
