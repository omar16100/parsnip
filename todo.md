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

Remaining: P1 protocol+dispatch, P2 client, P3 server wiring, P4 CLI wiring, P5 search+fulltext,
P6 render layer, P7 features/dist, P8 daemon, P9 tests, P10 docs.

## Pending

(none)
