# Parsnip Development TODO

## Completed

### Simplify Project UX (Invisible Projects)
- [x] Global search by default - `handlers.rs:144` changed `search_all.unwrap_or(false)` to `true`
- [x] Fix relation creation to use real entity IDs instead of fake IDs
- [x] Add `from_project_id`, `to_project_id` to Relation struct for cross-project support
- [x] Add `get_all_relations_all_projects()` to storage trait
- [x] Add `get_relations_for_entity_global()` to storage trait
- [x] Implement global queries in sqlite.rs
- [x] Implement global queries in redb.rs
- [x] Implement global queries in memory.rs
- [x] Update handler to support cross-project relations with auto-discovery
- [x] Update tool schema with `fromProjectId`, `toProjectId` parameters

## In Progress

### Remote client mode (branch `feat/remote-client-mode`)
Plan: [docs/28072026_remote_mode_plan.md](docs/28072026_remote_mode_plan.md). Fixes the redb exclusive-lock problem
(`Database already open`) by letting one daemon own the DB and everything else reach it over HTTP.

P0 preflight:
- [x] `--auth-token` moved to a global arg; removed the duplicate on `ServeArgs` (clap arg-ID collision)
- [x] `serve` gets a multi-threaded runtime; other commands keep `current_thread` for cold start
- [x] `config` and `completions` handled before storage opens, so they work while the DB is locked
- [x] Fixed pre-existing short-option collisions with globals that made `parsnip completions` panic
      for every shell (`-f` force/format, `-d` depth/data-dir, `-d` description/data-dir, `-p` target-project/project)
- [x] `JsonRpcRequest` gained `Serialize`, `JsonRpcResponse`/`JsonRpcError` gained `Deserialize`, plus `error.data`
- [x] `/health` advertises `capabilities` and `maxBodySize` for version-skew detection
- [x] SSE broadcast filtered to MCP methods only (`storage/*` responses stay point-to-point)
- [x] `run_sse_server` logs the actually-bound address, making `--port 0` usable
- [x] Delete the dead `ToolHandler` in `handlers.rs` (722 lines to 58)

P1 protocol + dispatch:
- [x] `remote/protocol.rs`: `storage/<trait_method>` names, param structs mirroring trait
      parameter names, `WireError {kind, detail}` with `StorageError` mapping both ways
- [x] `remote/dispatch.rs`: transport-agnostic `StorageDispatcher` covering all 22 storage methods
- [x] `StorageError::Remote` for remote-reported and transport failures
- [x] `storage/get_or_create_project` holds a lock across get-then-create, closing the race
      that silently orphans entities under a losing ProjectId (entity keys embed the project UUID)
- [x] Batch-size limits enforced server-side; content-shape limits deliberately not, so
      remote does not reject data local mode accepts

P2 client:
- [x] `reqwest` added to `[workspace.dependencies]` (no TLS features; Tailscale already encrypts)
- [x] `remote` / `remote-tls` cargo features; protocol and dispatcher always compiled
- [x] `RpcTransport` trait + `HttpTransport`, so the protocol is testable with no socket
- [x] `RemoteStorage` implements all 22 methods, overriding the three provided defaults
- [x] Batch chunking bounded by item count and serialized bytes
- [x] Connect/request timeouts (there were none anywhere before), 401 and 413 mapped to clear messages
- [x] `/health` capability check on connect, so version skew is one message not per-call failures
- [x] Conformance suite: identical assertions against MemoryStorage and RemoteStorage

P3 server wiring:
- [x] `?Sized` on McpServer and the seven sse.rs generic sites, so the server can be built
      over `Arc<dyn StorageBackend>`; compiled first try, no axum bound trouble
- [x] `storage/*` arm in `handle_request`, dispatching to StorageDispatcher
- [x] Errors returned with a structured `data` payload carrying the WireError kind
- [x] Body limit raised to 32MB on both `RequestBodyLimitLayer` and axum's `DefaultBodyLimit`
- [x] Contract tests: real router on an ephemeral port, real HTTP, real client, over
      `Arc<dyn StorageBackend>`; covers auth, capabilities, 5MB batches, error kinds,
      and that the MCP tool surface still works alongside the storage RPC

P4 CLI wiring:
- [x] `Storage` is now `dyn StorageBackend`, chosen at runtime; command handlers untouched
- [x] `AppContext` gains `remote`, so search can reach the search RPC without downcasting
- [x] `--server` (env `PARSNIP_SERVER`), `--local` to override, `server_url` config key
- [x] Precedence: `--server` > `PARSNIP_SERVER` > config; only `server_url` is read from
      config, since `--project` has a clap default and cannot be told apart from an explicit flag
- [x] redb lock failure now explains how to point at the daemon instead of the bare message
- [x] `sse` and `remote` are default features
- [x] Verified live: add/get/list, relation add --weight, traverse, project stats, search
      all work remotely with no handler changes

P5 search + fulltext:
- [x] `search/query` RPC handled server-side, mirroring the CLI's engine selection
- [x] `RemoteStorage::search`; SearchQuery already carries mode/filters/scope, so no new types
- [x] Rendering extracted so local and remote paths emit identical output
- [x] Full-text engine built per query in memory instead of opening the on-disk index
- [x] Verified remotely: exact, fuzzy, fulltext and hybrid all work

Correction: the predicted "on-disk index is permanently stale" bug does NOT reproduce.
0.1.0 returns current results in both directions. The real gain is that the index and its
lock are no longer created by every invocation (0.1.0 creates the index dir even for
`entity add`), and that remote mode must not open a local index at all. No user-visible
search behaviour change.

P9 end-to-end tests:
- [x] `crates/parsnip-cli/tests/remote_mode.rs`: real daemon process, real CLI binary, real HTTP
- [x] Acceptance criterion encoded: local fails with a lock error naming --server, remote succeeds
- [x] Full command surface, export/import round trip, auth, unreachable daemon, concurrency
- [x] Parity: local and remote stdout byte-identical across exact/fuzzy/fulltext/hybrid
- [x] `concurrent_clients_agree_on_one_project` caught that the atomic RPC was never wired
      into the CLI; three of four entities were being orphaned. Now fixed and asserted.

Pre-existing release bugs found and fixed along the way (both affect shipped 0.1.0):
- [x] `parsnip completions <shell>` panicked for every shell (short-option collisions)
- [x] `parsnip export` panicked at parse time: the global `--format` (String) and export's
      own `--format` (enum) shared a clap arg id

P7 (local install), P8 (daemon), P10 (docs):
- [x] Bumped to 0.2.0, release build, installed to `~/.local/bin/parsnip`
      (previous 0.1.0 binary kept aside, not used to serve)
- [x] launchd `com.omar.parsnip` on 127.0.0.1:8787, KeepAlive, token at `~/.parsnip/daemon-token`
- [x] Shell profile exports PARSNIP_SERVER/PARSNIP_AUTH_TOKEN; `server_url` also in config
- [x] Other Mac's MCP registration now proxies via PARSNIP_SERVER instead of opening the DB
- [x] docs/index.md, docs/c4model.md, docs/28072026_remote_mode_plan.md, AGENTS.md structure
- [x] Verified live: a real database, hybrid search ~70ms, writes work,
      exactly one process holds parsnip.redb

Correction: binding the daemon to the Tailscale address does not work on macOS. The TCP
connection is accepted and no response ever arrives, confirmed from both this host and the
other Mac, while 127.0.0.1 answers instantly. Daemon binds localhost; cross-machine access
keeps using the existing SSH arrangement. `tailscale serve` would be the way to expose it
directly.

P6 render layer:
- [x] `view/` module: view structs per command output + `Render` trait + `emit()`
- [x] `output.rs` deleted (it was dead code with "not yet implemented" stubs)
- [x] `-f json` works on every command; `-f csv` on row-shaped ones; detail/mutation shapes
      exit 2 with a message rather than faking a CSV
- [x] `--format` unified into one global enum, so `export` no longer needs its own
- [x] Sorted `project stats` breakdowns and `relation traverse` entity lists, which came
      out of HashMaps and printed in a different order on every run
- [x] Golden tests pin table output; 13 tests covering table, json and csv
- [x] Rebuilt, reinstalled, daemon restarted on the new binary

Also hardened the daemon: the plist now clears PARSNIP_SERVER and passes --local, because
launchctl carried the caller's environment in and the daemon tried to proxy to itself.

All phases complete. Not done: publishing (no tag, no crates.io, no tap update).

### Publish remote mode to main (27 Sep 2026)
Plan: [docs/27092026_publish_remote_mode_plan.md](docs/27092026_publish_remote_mode_plan.md).
- [x] Rebuilt the branch as `feat/remote-client-mode-v2` without an accidentally committed
      local backup; `.backups/` ignored; deploy plist log paths use a `YOUR_USERNAME` placeholder
- [x] Clippy clean on current stable (`sort_by_key`, `CARGO_BIN_EXE_parsnip`, reaped test daemon)
- [x] MCP tool calls and MCP proxies resolve projects through the atomic path
      (`StorageBackend::get_or_create_project`); `tests/project_race.rs` proves it
- [x] Constant-time token check, no-token warning, logs to stderr, `--local` beats `PARSNIP_SERVER`
- [x] README, site and llms.txt: correct crate name, no Homebrew or `parsnip.sh` commands,
      no vector mode, targets labelled as targets, Remote mode section
- [x] LICENSE-MIT year 2025, full Apache-2.0 text in LICENSE-APACHE
- [x] Review round 1 fixes: atomic `project create`/`import`, `save_project` rebind guard,
      pagination clamp (zero page size aborted the daemon), loopback Host guard, vetted bind
      host, token hidden in help and refused when empty, no SSE broadcast of search, multi-project
      scope, no client redirects, `--format` checked before mutations, MSRV 1.85
- [x] Review round 2 fixes: import `--merge` check on the resolved project, SQLite atomic
      project creation across processes, stricter Host parsing, IPv4 preference for `--host` names
- [ ] Follow-up: transactional or compare-and-swap entity updates (concurrent edits are last-write-wins)
- [ ] Follow-up: coordinate project deletion with concurrent writers
- [ ] Follow-up: route MCP SSE responses to the originating session instead of broadcasting
- [x] Follow-up: publish 0.2.0 (crates, tag, release, tap formula), then update the README install section (see Release 0.2.0 below)

### Security dependency updates (27 Sep 2026)
Plan: [docs/27092026_security_deps_plan.md](docs/27092026_security_deps_plan.md).
- [x] Lockfile: lz4_flex 0.11.6, oneshot 0.1.13, time 0.3.55, bytes 1.12.1, rand 0.8.8 and 0.9.5
      (Dependabot alerts 2 to 7; supersedes Dependabot PRs #1, #3, #4, #5, #7)
- [x] MSRV 1.88 (patched `time` needs it); README badge and AGENTS.md updated
- [x] `lru` alert 1 dismissed as tolerable risk: tantivy 0.22 pins `lru ^0.12` and never calls `IterMut`
- [ ] Follow-up: upgrade tantivy to 0.26 (brings `lru ^0.16.3`, `lz4_flex ^0.13`; index format and API changes)

### Homebrew claims on the published docs (27 Sep 2026)
Plan: [docs/27092026_homebrew_claims_plan.md](docs/27092026_homebrew_claims_plan.md).
- [x] `docs/todo.md` (served as `todo.html`): Homebrew marked planned, not published; install block matches `llms.txt`
- [x] `docs/spec.md` Appendix K: available channels separated from planned ones (no Homebrew, Docker, Nix; `parsnip` crate is unrelated)

### Release 0.2.0 (27 Sep 2026)
Plan: [docs/27092026_release_0_2_0_plan.md](docs/27092026_release_0_2_0_plan.md).
- [x] `release.yml`: verify job (tag = workspace version, CHANGELOG section, `cargo package --workspace`),
      publish fails on real errors (only "already on crates.io" is skipped, checked against the index),
      retries while dependencies reach the index, `checksums.sha256` on the release, notes from CHANGELOG
- [x] `update-homebrew.yml`: called from `release.yml` (a `GITHUB_TOKEN` release never triggered it),
      `workflow_dispatch` dry run, checksum and archive checks, formula test asserts the version
- [x] Crate metadata: readme, keywords, categories, homepage, license files in every package
- [x] `Cargo.lock`: yanked `chacha20 0.10.1` -> 0.10.2
- [x] `CHANGELOG.md` with 0.2.0 and 0.1.0
- [x] README, `llms.txt`, `docs/todo.md`, `docs/spec.md`: `cargo install parsnip-cli` gives 0.2.0 with remote mode
- [x] Tag v0.2.0, crates.io, release assets, formula verified (crates published from the maintainer's machine after CI's crates.io token was rejected; formula added in homebrew-tap#2)
- [ ] Replace `CARGO_REGISTRY_TOKEN` and `HOMEBREW_TAP_TOKEN` repo secrets so the next release publishes from CI
- [x] Homebrew install documented (README, site Homebrew tab, llms.txt, docs/spec.md, docs/todo.md)
- [ ] Follow-up (found in release review): `create_relations` advertises `fromProjectId`/`toProjectId`
      but `server.rs` ignores them; implement or drop them from the schema
- [ ] Follow-up: `add_observations` and `add_tags` skip the length checks `create_entities` applies
- [ ] Follow-up: `project use` ignores `--format json`

## Pending

(none)
