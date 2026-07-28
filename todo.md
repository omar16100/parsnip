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
Plan: `docs/28072026_parsnip_remote_mode_plan.md`. Fixes the redb exclusive-lock problem
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

Remaining: P6 render layer, P7 features/dist, P8 daemon, P9 tests, P10 docs.

## Pending

(none)
