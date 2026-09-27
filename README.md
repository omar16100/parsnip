# parsnip

**A local-first memory graph for AI assistants and knowledge workers.**

[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](#license)
[![Rust](https://img.shields.io/badge/rust-1.88%2B-orange.svg)](https://www.rust-lang.org/)

[Website](https://omar16100.github.io/parsnip/) · [Documentation](#cli-reference) · [Issues](https://github.com/omar16100/parsnip/issues)

---

## What is parsnip?

Parsnip is a single-binary graph database designed to store durable facts as **entities** and **relations**, enabling fast, reliable retrieval through integrated search and graph traversal.

**The problem:** Memory is scattered across chat logs, notes, and one-off files. AI assistants forget everything between sessions.

**The solution:** A unified, local-first knowledge graph that captures facts once and retrieves them quickly, even with typos, across projects, and offline.

```
┌─────────────────────────────────────────────────────────────┐
│                        PROJECT                              │
│  ┌─────────────┐                      ┌─────────────┐       │
│  │   Entity    │      works_at        │   Entity    │       │
│  │ John_Smith  │ ──────────────────▶  │  Acme_Corp  │       │
│  │ type:person │                      │type:company │       │
│  └─────────────┘                      └─────────────┘       │
│        │                                                    │
│        │ observations:                                      │
│        │  - "Works on distributed systems"                  │
│        │  - "Based in earth".                               │
│        │ tags: [engineer, senior]                           │
└─────────────────────────────────────────────────────────────┘
```

## Features

- **Local-first**: works offline, and by default your data stays on your machine. [Remote mode](#remote-mode) is opt-in.
- **Graph-native**: store knowledge as entities, relations, and observations.
- **Search modes**: exact, fuzzy (typo-tolerant) and full-text (BM25). `hybrid` is accepted and currently runs full-text.
- **Cross-project search**: query across all projects without mixing namespaces.
- **MCP integration**: 13 tools for AI assistants via the Model Context Protocol.
- **Graph traversal**: BFS traversal, Dijkstra shortest path, filters by entity and relation type.
- **Multiple backends**: ReDB (default) or SQLite; an in-memory backend is used in tests.
- **Remote mode**: one `parsnip serve` daemon owns the database and other CLI and MCP processes reach it over HTTP (unreleased, [build from source](#installation)).
- **Small and fast by design**: see the [performance targets](#performance-targets). They are design goals from `docs/spec.md`, not measured benchmarks.

## Installation

The binary is called `parsnip`; the crate is `parsnip-cli`. Note that `cargo install parsnip` installs an unrelated crate with the same name.

### From crates.io (0.1.0)

```bash
cargo install parsnip-cli
```

As of 27 Sep 2026 the latest published version is 0.1.0. It does not include remote mode or the fixes listed in [todo.md](todo.md).

### Latest from source (includes remote mode)

```bash
cargo install --git https://github.com/omar16100/parsnip parsnip-cli
```

Or from a clone:

```bash
git clone https://github.com/omar16100/parsnip.git
cd parsnip
cargo build --release -p parsnip-cli   # binary at target/release/parsnip
```

### Prebuilt binary (0.1.0)

```bash
curl -fsSL https://raw.githubusercontent.com/omar16100/parsnip/main/install.sh | sh
```

Downloads the latest [GitHub release](https://github.com/omar16100/parsnip/releases) for Linux x86_64 or macOS (x86_64, arm64) into `~/.local/bin`.

### Feature Flags

Default features in source builds: `redb`, `fulltext`, `sse`, `remote`. (0.1.0 on crates.io defaults to `redb`, `fulltext`.)

| Feature | What it adds |
|---------|--------------|
| `redb` | ReDB storage (default backend) |
| `sqlite` | SQLite storage. Only used when `redb` is off, so disable default features |
| `fulltext` | Tantivy full-text search (also used by `--mode hybrid`) |
| `sse` | HTTP/SSE transport for `parsnip serve` |
| `remote` | HTTP client for [remote mode](#remote-mode) |
| `remote-tls` | `https://` daemon URLs (rustls) |

```bash
# SQLite instead of ReDB
cargo install --git https://github.com/omar16100/parsnip parsnip-cli \
  --no-default-features --features sqlite,fulltext,sse,remote
```

## Quick Start

```bash
# Create a project
parsnip project create work --description "Work knowledge"

# Add entities
parsnip -p work entity add John_Smith -t person -o "Senior engineer at Acme" --tag engineer
parsnip -p work entity add Acme_Corp -t company -o "Tech company in Singapore"

# Create a relation
parsnip -p work relation add John_Smith Acme_Corp -t works_at

# Search
parsnip -p work search John
parsnip -p work search "engineer" --mode fuzzy
parsnip -p work search "distributed systems" --mode fulltext

# Traverse the graph
parsnip -p work relation traverse John_Smith --depth 2

# Export for backup
parsnip -p work export -o backup.json
```

## CLI Reference

### Global Options

| Option | Description |
|--------|-------------|
| `-p, --project <NAME>` | Project namespace (default: "default") |
| `-d, --data-dir <PATH>` | Custom data directory |
| `-f, --format <FMT>` | Output format: table, json, csv (graphml is export only) |
| `-v, --verbose` | Increase verbosity (-v, -vv, -vvv) |
| `-q, --quiet` | Only log errors (command output is unchanged) |
| `--server <URL>` | Use a parsnip daemon instead of the local database (env `PARSNIP_SERVER`) |
| `--local` | Use the local database for this command, overriding any server setting |
| `--auth-token <TOKEN>` | Bearer token for the daemon (env `PARSNIP_AUTH_TOKEN`) |

### Entity Commands

```bash
# Create entity with observations and tags
parsnip entity add <NAME> -t <TYPE> -o "observation" --tag tag1 --tag tag2

# List entities with filters
parsnip entity list [--type <TYPE>] [--tag <TAG>] [--limit <N>]

# Get entity details
parsnip entity get <NAME>

# Add observation to existing entity
parsnip entity observe <NAME> "new fact"

# Delete entity
parsnip entity delete <NAME> [--force]
```

### Relation Commands

```bash
# Create relation
parsnip relation add <FROM> <TO> -t <TYPE> [-w <WEIGHT>]

# List relations
parsnip relation list [--from <NAME>] [--to <NAME>] [--type <TYPE>]

# Delete relation
parsnip relation delete <FROM> <TO> -t <TYPE>

# Traverse graph (BFS, default depth 2)
parsnip relation traverse <START> [--depth <DEPTH>] [--direction outgoing|incoming|both]
parsnip relation traverse <START> --entity-types person --relation-types works_at

# Find shortest path (--weighted uses Dijkstra)
parsnip relation find-path <FROM> <TO> [--weighted] [--relation-types <TYPES>] [--max-depth <N>]
```

### Search Commands

```bash
# Basic search
parsnip search <QUERY>

# Search modes
parsnip search <QUERY> --mode exact      # Substring match
parsnip search <QUERY> --mode fuzzy      # Typo-tolerant
parsnip search <QUERY> --mode fulltext   # BM25 ranking
parsnip search <QUERY> --mode hybrid     # Currently the same as fulltext

# Filter by tags
parsnip search --tag engineer --tag senior

# Cross-project search
parsnip search <QUERY> --all-projects

# Limit results (default 100)
parsnip search <QUERY> --limit 20
```

### Project Commands

```bash
# List all projects
parsnip project list

# Create project
parsnip project create <NAME> [--description "description"]

# Record a default project in the config file
# (not yet applied to other commands; pass -p <NAME> instead)
parsnip project use <NAME>

# Get project stats
parsnip project stats

# Delete project
parsnip project delete <NAME> [--force]
```

### Import/Export Commands

```bash
# Export single project
parsnip export -o backup.json

# Export all projects
parsnip export --all-projects -o full-backup.json

# Import (projects keep the names recorded in the file)
parsnip import data.json

# Import everything into one project
parsnip import data.json --target-project newproject

# Merge with existing data
parsnip import data.json --merge
```

### Server Commands

```bash
# Start MCP server (stdio)
parsnip serve

# Start MCP server over HTTP/SSE on 127.0.0.1:3000 (needs the `sse` feature,
# default in source builds)
parsnip serve -t sse --port 3000

# Listening on a non-localhost address needs --allow-remote and a token
parsnip --auth-token "$TOKEN" serve -t sse --host 0.0.0.0 --allow-remote
```

## Remote Mode

The default ReDB backend lets exactly one process open the database, so a long-running `parsnip serve` locks every other `parsnip` command out. Remote mode fixes that: one daemon owns the database, and CLI commands and MCP servers reach it over HTTP. It is in the source tree but not in the 0.1.0 release, so [install from source](#latest-from-source-includes-remote-mode) to use it.

**1. Start the daemon** (the only process that opens the database):

```bash
mkdir -p ~/.parsnip && (umask 077 && openssl rand -hex 32 > ~/.parsnip/daemon-token)
export PARSNIP_AUTH_TOKEN="$(cat ~/.parsnip/daemon-token)"
parsnip --local serve --transport sse --host 127.0.0.1 --port 8787
```

`--local` keeps the daemon on the local database even if `PARSNIP_SERVER` is set in its environment. Otherwise it would try to proxy to itself.

**2. Point clients at it**, in order of precedence:

```bash
parsnip --server http://127.0.0.1:8787 entity list   # per command
export PARSNIP_SERVER=http://127.0.0.1:8787          # per shell
parsnip config set server_url http://127.0.0.1:8787  # persistent, lowest precedence
```

Clients send `PARSNIP_AUTH_TOKEN` (or `--auth-token`) as a bearer token. `parsnip --local <command>` bypasses the daemon for one command, which only works while the daemon is stopped.

**3. MCP clients** can run the stdio server as a proxy that never opens the database:

```json
{
  "mcpServers": {
    "parsnip": {
      "command": "parsnip",
      "args": ["serve"],
      "env": {
        "PARSNIP_SERVER": "http://127.0.0.1:8787",
        "PARSNIP_AUTH_TOKEN": "<token>"
      }
    }
  }
}
```

Security notes:

- `serve` binds `127.0.0.1` by default. Without a token, any local process that can reach the port can read and write the whole graph, so set one even on localhost. A tokenless daemon also rejects requests whose `Host` header is not a loopback name, which blocks DNS-rebinding attacks from web pages.
- A `--host` that does not resolve only to loopback addresses requires both `--allow-remote` and a token, and an empty token is refused. Traffic is plain HTTP: only expose the port over an encrypted network (for example a VPN) or behind an HTTPS proxy, using the `remote-tls` feature for `https://` URLs. Clients do not follow redirects.
- `GET /health` is unauthenticated and reports the version and capabilities. Everything else needs the token when one is set.

Every CLI command works remotely, and `config` and `completions` never touch the database. Known limits:

- `relation traverse`, `relation find-path` and `search --include-relations` fetch whole projects to the client.
- A large `import` is sent in several requests, so an interrupted import can leave partial data.
- Updates are read-modify-write from the client: if two clients change the same entity at the same moment (for example both adding an observation), the last write wins.
- Deleting a project while another client is writing to it can leave that client's new entities under the deleted project.

New projects are created atomically on the daemon, so concurrent clients never split one project name across two ids. See [docs/c4model.md](docs/c4model.md) for the design and [deploy/com.omar.parsnip.plist](deploy/com.omar.parsnip.plist) for an example macOS launchd job.

## MCP Integration

Parsnip includes a Model Context Protocol (MCP) server that gives AI assistants persistent memory.

### Claude Desktop Setup

Add to your `claude_desktop_config.json`:

```json
{
  "mcpServers": {
    "parsnip": {
      "command": "parsnip",
      "args": ["serve"]
    }
  }
}
```

### Available MCP Tools

| Tool | Description |
|------|-------------|
| `search_knowledge` | Search entities (exact, fuzzy, or full-text) |
| `create_entities` | Batch create entities with observations and tags |
| `add_observations` | Add facts to existing entities |
| `create_relations` | Create typed relations between entities |
| `delete_entities` | Remove entities (cascades relations) |
| `delete_observations` | Remove specific observations |
| `delete_relations` | Remove relations |
| `read_graph` | Get complete project graph |
| `open_nodes` | Retrieve specific entities by name |
| `add_tags` | Add tags to entities |
| `remove_tags` | Remove tags from entities |
| `traverse_graph` | BFS/Dijkstra traversal with filters |
| `list_projects` | List projects with entity and relation counts |

## Search Modes

| Mode | Description | Use Case |
|------|-------------|----------|
| **Exact** | Substring matching | Precise queries, known names |
| **Fuzzy** | Nucleo-based, typo-tolerant | Misspellings, partial recall |
| **Full-text** | Tantivy BM25 ranking | Natural language queries |
| **Hybrid** | Currently runs the full-text engine | Same as full-text for now |

`search --fuzzy` is shorthand for `--mode fuzzy`. `--threshold` is accepted but not yet applied by the fuzzy engine.

## Storage Backends

### ReDB (Default)

Embedded key-value store with ACID transactions. Zero external dependencies.

```bash
# Data stored at:
# macOS: ~/Library/Application Support/parsnip/parsnip.redb
# Linux: ~/.local/share/parsnip/parsnip.redb
```

### SQLite

Relational backend compatible with SQL tools. Stored as `parsnip.sqlite` in the data directory.

```bash
cargo install --git https://github.com/omar16100/parsnip parsnip-cli \
  --no-default-features --features sqlite,fulltext,sse,remote
```

### Memory

In-memory storage for testing. No persistence.

```bash
# Used automatically in tests
```

## Configuration

### Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `PARSNIP_SERVER` | Daemon URL for [remote mode](#remote-mode) (same as `--server`) | unset (local database) |
| `PARSNIP_AUTH_TOKEN` | Bearer token for the daemon (same as `--auth-token`) | unset |
| `RUST_LOG` | Log filter, overrides `-v`/`-q` (e.g. `debug`, `parsnip=trace`) | `warn` |
| `XDG_CONFIG_HOME` | If set, the config file is `$XDG_CONFIG_HOME/parsnip/config.toml` | unset |

Logs go to stderr.

### Config File

`~/.parsnip/config.toml` (see `parsnip config path`). Manage it with `parsnip config list|get|set|init`. Keys: `default_project`, `data_dir`, `log_level`, `output_format`, `server_url`. Only `server_url` is currently applied to commands; the others are stored but not yet read.

### Data Directory Locations

| Platform | Path |
|----------|------|
| macOS | `~/Library/Application Support/parsnip/` |
| Linux | `~/.local/share/parsnip/` |
| Windows | `%APPDATA%\parsnip\` |

## Performance Targets

Design targets from [docs/spec.md](docs/spec.md). They have not been benchmarked in this repository.

| Operation | Target |
|-----------|--------|
| Cold start | <10ms |
| Entity create | <1ms |
| Batch create (100) | <10ms |
| Exact search (10k entities) | <5ms |
| Fuzzy search (10k entities) | <20ms |
| Full-text search (10k entities) | <10ms |
| Cross-project search (100k) | <100ms |
| Graph traversal (depth 3) | <50ms |
| Binary size (stripped) | <15MB |
| Idle memory | <20MB |

## Architecture

```
parsnip/
├── crates/
│   ├── parsnip-core/       # Core types: Entity, Relation, Observation, Project
│   ├── parsnip-storage/    # Storage backends: ReDB, SQLite, Memory, Remote (storage RPC)
│   ├── parsnip-search/     # Search engines: Exact, Fuzzy, FullText
│   ├── parsnip-cli/        # CLI binary with all commands
│   └── parsnip-mcp/        # MCP server (13 tools) and the HTTP/SSE daemon
├── deploy/                 # Example launchd job for the daemon
└── docs/
    ├── index.md            # Documentation index
    ├── c4model.md          # Architecture (source of truth)
    ├── spec.md             # Original specification
    └── index.html          # Website
```

Integration tests live in `crates/<crate>/tests/`.

### Crate Dependencies

```
parsnip-cli
    ├── parsnip-core
    ├── parsnip-storage
    │   └── parsnip-core
    ├── parsnip-search
    │   └── parsnip-core
    └── parsnip-mcp
        ├── parsnip-core
        ├── parsnip-storage
        └── parsnip-search
```

## Contributing

Contributions are welcome! Please:

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Make your changes
4. Run tests (`cargo test --workspace`)
5. Submit a pull request

### Development Setup

```bash
git clone https://github.com/omar16100/parsnip.git
cd parsnip
cargo build
cargo test
```

### Code Style

- Format with `cargo fmt`
- Lint with `cargo clippy --workspace --all-targets -- -D warnings`
- Test coverage target (from the spec): >80%

## License

Dual-licensed under [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE) at your option.

---

**Built with Rust** · [Website](https://omar16100.github.io/parsnip/) · [GitHub](https://github.com/omar16100/parsnip)
