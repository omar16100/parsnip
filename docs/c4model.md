# Architecture (C4)

Source of truth for parsnip's containers, components and data flows. Read this before
making architecture changes, and update it as part of them.

## Context

Parsnip is a knowledge graph for AI assistants. Two kinds of consumer use it:

- **People**, through the `parsnip` CLI.
- **LLM clients** (Claude Code and similar), through an MCP server.

Both may run on several machines on one Tailscale network, against a single graph.

## The constraint that shapes everything

The default storage backend is redb, which takes an **exclusive process-wide lock**: exactly
one process may open the database. Any long-lived `parsnip serve` therefore locks every
other process out of the file. This is why the architecture has a daemon at its centre
rather than letting each invocation open the database.

## Containers

```
                    ┌───────────────────────────────────────────────┐
                    │  parsnip daemon  (launchd: com.omar.parsnip)   │
                    │  `parsnip serve --transport sse`               │
                    │                                                │
                    │  SOLE owner of parsnip.redb                    │
                    │                                                │
                    │  axum router on 127.0.0.1:8787                 │
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
                                                     spawned over SSH from
                                                     another tailnet Mac
```

A CLI invocation with no server configured opens the database directly instead. That is
the single-machine, no-daemon mode, and it is still the default when nothing is set.

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
- **Project resolution is server-side.** `storage/get_or_create_project` holds a lock across
  get-then-create. Doing it client-side loses data: entity keys embed the project UUID, so
  the loser of a race leaves rows under an id no name resolves to.
- **Validation.** The dispatcher enforces batch-size limits, which bound per-request memory
  on a shared daemon. It deliberately does not enforce name and observation lengths, because
  the local path does not either, and diverging would make remote mode reject data local
  mode accepts.
- **Two body limits.** `RequestBodyLimitLayer` and axum's `DefaultBodyLimit` are enforced
  independently; both are set to 32MB. Clients also chunk batches.
- **The SSE broadcast** is filtered to MCP methods. It is the only delivery path to `/sse`
  subscribers, so it cannot simply be removed, but storage RPC responses must not be pushed
  at every subscriber.
- **Binding.** The daemon binds 127.0.0.1. Binding directly to a Tailscale address on macOS
  accepts the connection and then never answers; use `tailscale serve` in front of the port
  for direct cross-machine HTTP.
