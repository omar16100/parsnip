# Remote client mode

Status: **deployed** (P0-P5, P7 local install, P8, P9, P10 complete; P6 outstanding).
Branch: `feat/remote-client-mode`.

## Context

Every `parsnip` CLI call on the host machine failed with:

```
Error: Database error: Database already open. Cannot acquire lock.
```

Not a stale lockfile. redb takes an exclusive process-wide lock, and the CLI opened the
database on every invocation, so a long-lived `parsnip serve` locked the CLI out
permanently. The trigger was an MCP server spawned over Tailscale SSH from another Mac,
which had been holding the database for about 18 hours.

Goal: exactly one process owns the database; every other consumer reaches it over HTTP.
Doing that also gives cross-machine access, which the SSH arrangement was approximating.

## Approach

Remote the `StorageBackend` trait rather than the MCP tool surface. `AppContext.storage`
became `Arc<dyn StorageBackend>`, so command handlers are untouched and no command loses
capability. See [c4model.md](c4model.md) for the resulting architecture and the reasoning
against the alternative.

## What shipped

| Phase | Outcome |
|---|---|
| P0 | `config`/`completions` no longer open the database; `serve` gets a multi-threaded runtime; `--auth-token` moved to a global; JSON-RPC types made usable by clients; `/health` advertises capabilities; SSE broadcast filtered; dead `ToolHandler` deleted (722 lines to 58) |
| P1 | `remote/protocol.rs` and `remote/dispatch.rs`; `storage/get_or_create_project`; batch-size validation |
| P2 | `RemoteStorage` over HTTP, batch chunking, timeouts, capability check; conformance suite |
| P3 | `?Sized` through `McpServer` and the SSE router; `storage/*` served alongside `tools/*`; body limits raised to 32MB; first tests the SSE layer has had |
| P4 | Runtime storage selection: `--server`, `PARSNIP_SERVER`, `server_url`, `--local`; actionable lock error |
| P5 | `search/query` RPC; full-text index built per query instead of held open |
| P7 | Built and installed 0.2.0 to `~/.local/bin/parsnip` (no crates.io or tap release) |
| P8 | launchd daemon `com.omar.parsnip`; both Macs' access routed through it |
| P9 | End-to-end tests including the acceptance criterion and the concurrency proof |
| P10 | This document, `index.md`, `c4model.md`, AGENTS.md structure section |

P6 (a shared render layer so `-f json` and `-f csv` work) is **not** done. `output.rs`
remains dead code and the global `--format` is still ignored by the commands.

## Bugs found along the way

Two crashes in the released 0.1.0, both surfaced by tests written for this work:

- `parsnip completions <shell>` panicked for every shell. Four short options collided with
  global ones (`-f` force/format, `-d` depth/data-dir, `-d` description/data-dir,
  `-p` target-project/project). Long forms are unchanged; only the shorts were removed.
- `parsnip export` panicked at parse time with "Mismatch between definition and access of
  `format`": the global `--format` (String) and export's own `--format` (enum) shared a
  clap arg id. `--format` is no longer `global`.

One data-loss race, introduced by making the graph shared and caught before release:
concurrent clients creating the same new project each minted a different `ProjectId`, and
since entity keys embed that UUID, the losers' entities became unreachable. Fixed with a
server-side atomic `get_or_create_project` and asserted by
`concurrent_clients_agree_on_one_project`.

## Corrections to the original plan

- The plan asserted the on-disk full-text index was permanently stale, so hybrid search
  was missing recent entities. **This does not reproduce.** Tested directly against 0.1.0:
  hybrid search returns current results in both directions. The index rebuilds whenever the
  reader sees no documents, and hits are filtered against the entity slice passed in. The
  change to a per-query in-memory index is justified by the lock it removes (0.1.0 creates
  the index directory even for `entity add`) and by remote mode, not as a correctness fix.
- The plan called for binding the daemon to the Tailscale address. That **does not work on
  macOS**: the TCP connection is accepted and no response ever arrives, verified with curl
  both locally and from another tailnet Mac, while the same binary on 127.0.0.1 answers
  instantly. The daemon binds 127.0.0.1; cross-machine access goes through the existing SSH
  arrangement. Use `tailscale serve` if direct cross-machine HTTP is wanted.
- `?Sized` propagation was flagged as the fiddly part with an hour budgeted. It compiled on
  the first attempt.

## Deployment

- Binary: `~/.local/bin/parsnip` (0.2.0). Any older 0.1.0 install must not be used to
  serve; `~/.local/bin` must precede it on PATH.
- Daemon: `~/Library/LaunchAgents/com.omar.parsnip.plist` (mode 600), `RunAtLoad`,
  `KeepAlive`, `ThrottleInterval 10`. Logs to `~/Library/Logs/parsnip-daemon.{out,err}.log`.
  A copy of the plist lives in [`deploy/com.omar.parsnip.plist`](../deploy/com.omar.parsnip.plist).
- Token: `~/.parsnip/daemon-token` (mode 600), generated with `openssl rand -hex 32`.
- Shell: `PARSNIP_SERVER` and `PARSNIP_AUTH_TOKEN` exported from the shell profile.
  `server_url` is also set in `~/.parsnip/config.toml`
  as a fallback for shells without the exports.
- Other Mac: its MCP registration now runs the stdio server with `PARSNIP_SERVER` set, so it
  proxies to the daemon instead of opening the database. The token is read from the daemon
  host's token file rather than stored in the remote config.

### Operating it

```sh
launchctl print gui/$(id -u)/com.omar.parsnip     # status
curl -s http://127.0.0.1:8787/health              # liveness and capabilities
launchctl bootout   gui/$(id -u)/com.omar.parsnip # stop
launchctl bootstrap gui/$(id -u) ~/Library/LaunchAgents/com.omar.parsnip.plist
parsnip --local <cmd>                             # bypass the daemon (needs it stopped)
```

If a command reports the database is already open, the daemon holds it; that is the normal
state. Use the daemon, or stop it first.

## Verification performed

- `cargo test --workspace`: 80 tests pass, including the conformance suite (local and remote
  backends held to identical assertions), the SSE contract tests, and the end-to-end suite.
- Acceptance criterion, as a test and by hand: with the daemon holding the lock, a local
  command fails with an error naming `--server`, and the same command through the daemon
  succeeds.
- Against a real database, after an export backup:
  hybrid search, project list, entity get, and a write, all through the daemon. Hybrid
  search completes in roughly 70ms, so the per-query reindex is not a concern at this size.
- From the other Mac: the proxy command answers `initialize` and returns real
  `search_knowledge` results, without opening the database.
- `lsof` confirms exactly one process holds `parsnip.redb`: the daemon.

## Follow-ups

- **P6, the render layer.** `output.rs` is dead code with "not yet implemented" stubs, and
  every handler prints inline, so `-f json` and `-f csv` do nothing. Needs a shared view and
  render path, with golden tests pinning current table output.
- **Chatty paths.** `traverse`, `find-path` and `--include-relations` still pull whole
  projects to the client. Measure before adding RPCs for them.
- **Import atomicity.** The local redb backend commits a batch in one transaction; over the
  wire a large import becomes several, so an interrupted import can leave partial state.
- **Head-of-line blocking.** `Mutex<Database>` serializes reads as well as writes, so a long
  import from one client stalls another's reads.
- **Release.** Nothing was published. A real release means tagging 0.2.0, publishing five
  crates, and adding a formula to the tap.
