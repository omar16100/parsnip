# Architecture (C4)

Source of truth for parsnip's containers, components and data flows. Read this before
making architecture changes, and update it as part of them.

## Context

Parsnip is a knowledge graph for AI assistants. Two kinds of consumer use it:

- **People**, through the `parsnip` CLI.
- **LLM clients** (Claude Code and similar), through an MCP server.

Both may run on several machines (for example on one private network such as a Tailscale
tailnet) against a single graph, through remote mode. Without remote mode everything runs
on one machine against a local database file.

## The constraint that shapes everything

The default storage backend is redb, which takes an **exclusive process-wide lock**: exactly
one process may open the database. Any long-lived `parsnip serve` therefore locks every
other process out of the file. This is why the architecture has a daemon at its centre
rather than letting each invocation open the database.

## Containers

```
                    ┌───────────────────────────────────────────────┐
                    │  parsnip daemon (example: deploy/*.plist)      │
                    │  `parsnip serve --transport sse`               │
                    │                                                │
                    │  SOLE owner of parsnip.redb                    │
                    │                                                │
                    │  axum router, 127.0.0.1:<port> by default      │
                    │    GET  /health   (no auth; capabilities)      │
                    │    GET  /sse      (MCP event stream)           │
                    │    POST /message  (JSON-RPC, bearer auth)      │
                    │       ├── tools/*     → MCP tool surface       │
                    │       ├── storage/*   → StorageDispatcher      │
                    │       └── search/query→ search engines         │
                    └───▲───────────────▲───────────────▲────────────┘
                        │               │               │
              storage/* │      tools/*  │      storage/*│
                        │               │               │
        ┌───────────────┴──┐  ┌─────────┴────────┐  ┌───┴─────────────────┐
        │ parsnip CLI      │  │ MCP client       │  │ MCP proxy           │
        │ --server URL     │  │ (Claude Code)    │  │ `serve --transport  │
        │ RemoteStorage    │  │                  │  │  stdio` +           │
        │                  │  │                  │  │ PARSNIP_SERVER      │
        └──────────────────┘  └──────────────────┘  └─────────────────────┘
                                                     e.g. spawned over SSH
                                                     from another machine
```

A CLI invocation with no server configured opens the database directly instead. That is
the single-machine, no-daemon mode, and it is still the default when nothing is set.

| Container | Technology | Responsibility |
|---|---|---|
| CLI (`parsnip <command>`) | Rust binary, clap, current-thread tokio runtime | Command surface; opens the database or uses `RemoteStorage` |
| MCP server, stdio (`parsnip serve`) | Same binary | MCP tools for one LLM client; local database or proxy via `PARSNIP_SERVER` |
| Daemon (`parsnip serve --transport sse`) | Same binary, axum, multi-thread tokio runtime | Sole database owner; serves `tools/*`, `storage/*`, `search/query` |
| Database | redb file `parsnip.redb` (or `parsnip.sqlite` with the `sqlite` feature) in the data directory | Entities, relations, projects |
| Search | In-process library (`parsnip-search`): exact, nucleo fuzzy, tantivy full-text built in memory per query | Runs wherever the data is: in the CLI locally, in the daemon remotely |
| Config | `~/.parsnip/config.toml` (or `$XDG_CONFIG_HOME/parsnip/`) | Only `server_url` is read at runtime |

## Components

| Crate | Responsibility |
|---|---|
| `parsnip-core` | Domain types (Entity, Relation, Project, SearchQuery), traversal, validation limits |
| `parsnip-storage` | `StorageBackend` trait and its implementations: redb, sqlite, memory, and remote |
| `parsnip-storage::remote` | The storage RPC. `protocol` (wire format), `dispatch` (server side), `client` (`RemoteStorage`) |
| `parsnip-search` | Exact, fuzzy and full-text (tantivy) search engines |
| `parsnip-mcp` | MCP server: tool surface, JSON-RPC transport, SSE/HTTP router; hosts the storage dispatcher |
| `parsnip-cli` | Command surface, output, config; chooses local or remote storage at runtime |

### Why remoting happens at the storage trait

`AppContext.storage` is `Arc<dyn StorageBackend>`. Every command handler calls through that
trait, so pointing the CLI at a daemon requires no changes in the handlers, and no command
loses capability. The alternative considered was routing the CLI through the MCP tool
surface; that surface has no equivalent for relation weights, entity timestamps, filtered
listing, project create/delete, import or export, and its mutation tools return prose
rather than data.

The cost is that the trait is a fine-grained RPC boundary, so remote mode is chatty.
`search/query` exists because search is the hot path: it would otherwise pull the entire
entity set across the network per query, and full-text could not work at all, since the
index belongs to whoever owns the data. `traverse`, `find-path` and `--include-relations`
still fetch whole projects client-side.

## Data flows

**Local CLI command** (no server configured)
```
parsnip entity add → AppContext::new opens redb → handler → ctx.storage.save_entity
```

**Remote CLI command**
```
parsnip entity add → RemoteStorage → POST /message {"method":"storage/save_entity"}
   → StorageDispatcher → redb (in the daemon)
```

**LLM tool call on the daemon**
```
MCP client → POST /message {"method":"tools/call"} → McpServer tool handler
   → project resolved via StorageDispatcher::get_or_create_project (same lock as storage/*)
   → redb
```

**LLM tool call through the SSH proxy**
```
Claude on another Mac → ssh → `parsnip serve --transport stdio` with PARSNIP_SERVER set
   → McpServer<RemoteStorage> → POST /message → daemon → redb
```
The proxy never opens the database, which is what stops it from taking the lock.

**Search**
```
remote: RemoteStorage::search → search/query → daemon picks the engine → Vec<Entity>
local:  fetch entities → engine chosen in-process → Vec<Entity>
```
Both paths render through the same function, so their output is identical.

## Cross-cutting decisions

- **Errors over the wire.** `StorageError` wraps io, serde and six redb types and cannot be
  serialized, so it travels as `WireError {kind, detail}` in the JSON-RPC error `data`, and
  the client rebuilds the variant. `detail` is the variant payload, not its `Display`
  output, or the prefix is applied twice.
- **Project resolution is server-side and single-path.** `StorageBackend::get_or_create_project`
  is a provided trait method (plain get-then-create). `StorageDispatcher` calls it under a
  lock, and the MCP tool surface resolves projects through the dispatcher, so tool calls and
  storage RPC clients in one daemon share that lock. `RemoteStorage` overrides the method
  with the atomic `storage/get_or_create_project` call, so CLI clients and MCP proxies never
  do get-then-create over the wire. Doing it client-side loses data: entity keys embed the
  project UUID, so the loser of a race leaves rows under an id no name resolves to.
  `project create` and `import` resolve through the same method. The dispatcher's
  `storage/save_project` refuses to bind an existing name to a different id. SQLite, which
  several processes may open at once, overrides the method with `INSERT OR IGNORE` plus a
  read-back so it is atomic across processes too.
- **Validation.** The dispatcher enforces batch-size limits, which bound per-request memory
  on a shared daemon. It deliberately does not enforce name and observation lengths, because
  the local path does not either, and diverging would make remote mode reject data local
  mode accepts.
- **Two body limits.** `RequestBodyLimitLayer` and axum's `DefaultBodyLimit` are enforced
  independently; both are set to 32MB. Clients also chunk batches.
- **The SSE broadcast** is filtered to MCP methods. It is the only delivery path to `/sse`
  subscribers, so it cannot simply be removed, but storage RPC responses must not be pushed
  at every subscriber.
- **Binding.** `serve` binds 127.0.0.1 by default. Any other host needs `--allow-remote` and a
  token. Binding directly to a Tailscale address on macOS was observed to accept the
  connection and never answer; `tailscale serve` in front of the port is the tested way to
  expose it across machines.
- **Auth.** Optional bearer token (`--auth-token` / `PARSNIP_AUTH_TOKEN`), compared without
  an early exit, never shown in `--help`, and refused if empty. `/health` is
  unauthenticated. Without a token the daemon logs a warning and accepts only loopback
  `Host` headers (DNS-rebinding guard). The bind host is resolved first and must be
  loopback-only unless `--allow-remote` and a token are given. Transport is plain HTTP
  unless fronted by TLS (`remote-tls` on clients); clients never follow redirects.
- **Untrusted query input.** `search/query` deserializes a `SearchQuery` from the network,
  so engines use `Pagination::limit()`/`offset()` (clamped, saturating) rather than the raw
  fields; a zero page size used to panic tantivy and, with `panic = "abort"`, kill the daemon.
  A multi-project scope is applied before searching.
- **Output formats** are checked before a command runs, so an unsupported `--format` never
  follows a completed mutation.
- **SSE broadcast** carries only MCP responses; `storage/*` and `search/query` responses stay
  point-to-point.
- **Logs** go to stderr, so stdout carries only command output or the stdio JSON-RPC stream.

## Change log

| Date | Change |
|---|---|
| 28 Jul 2026 | Remote client mode: storage RPC (`storage/*`, `search/query`), `RemoteStorage`, runtime storage selection, daemon as sole database owner. |
| 27 Sep 2026 | MCP tool surface, MCP proxies, `project create` and `import` resolve projects through the same atomic path as the storage RPC; `save_project` cannot rebind a name. Constant-time token check, hidden in help, empty token refused, no-token warning and loopback `Host` guard; bind host resolved and vetted before storage opens; no client redirects. Pagination clamped for network input; multi-project search scope honoured; `search/query` not broadcast. Format checked before mutations. SQLite project creation atomic across processes. Logs to stderr (no ANSI codes when not a terminal); `--local` overrides an exported `PARSNIP_SERVER`. MSRV 1.85. |
