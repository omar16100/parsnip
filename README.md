# parsnip

**A local-first memory graph for AI assistants and knowledge workers.**

[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.75%2B-orange.svg)](https://www.rust-lang.org/)

[Website](https://omar16100.github.io/parsnip/) · [Documentation](#cli-reference) · [Issues](https://github.com/omar16100/parsnip/issues)

---

## What is parsnip?

Parsnip is a single-binary graph database designed to store durable facts as **entities** and **relations**, enabling fast, reliable retrieval through integrated search and graph traversal.

**The problem:** Memory is scattered across chat logs, notes, and one-off files. AI assistants forget everything between sessions.

**The solution:** A unified, local-first knowledge graph that captures facts once and retrieves them instantly—even with typos, across projects, and offline.

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

- **Local-First** — Completely offline. Your data never leaves your machine. Private by design, portable by nature.
- **Graph-Native** — Store knowledge as entities, relations, and observations. True graph semantics, not SQL with JSON blobs.
- **5 Search Modes** — Exact, fuzzy (typo-tolerant), full-text (BM25), hybrid (combined), and vector (semantic).
- **Cross-Project Search** — Query across all projects without mixing namespaces.
- **MCP Integration** — 12 tools for AI assistants via Model Context Protocol. Works with Claude Desktop.
- **Graph Traversal** — BFS, Dijkstra shortest path, filtered traversal by entity/relation types.
- **Multiple Backends** — ReDB (default), SQLite, or in-memory storage.
- **Fast** — <10ms cold start, <5ms search on 10k entities, <15MB binary.

## Installation

### From Cargo (Recommended)

```bash
cargo install parsnip
```

### From Source

```bash
git clone https://github.com/omar16100/parsnip.git
cd parsnip
cargo build --release
```

### Feature Flags

```bash
# Default (ReDB storage)
cargo install parsnip

# With SQLite backend
cargo install parsnip --features sqlite

# With SSE/HTTP transport for MCP
cargo install parsnip --features sse

# With vector/semantic search
cargo install parsnip --features vector
```

## Quick Start

```bash
# Create a project
parsnip project create work -d "Work knowledge"

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
parsnip -p work relation traverse John_Smith -d 2

# Export for backup
parsnip -p work export -o backup.json
```

## CLI Reference

### Global Options

| Option | Description |
|--------|-------------|
| `-p, --project <NAME>` | Project namespace (default: "default") |
| `-d, --data-dir <PATH>` | Custom data directory |
| `-f, --format <FMT>` | Output format: table, json, csv |
| `-v, --verbose` | Increase verbosity (-v, -vv, -vvv) |
| `-q, --quiet` | Suppress non-error output |

### Entity Commands

```bash
# Create entity with observations and tags
parsnip entity add <NAME> -t <TYPE> -o "observation" --tag tag1 --tag tag2

# List entities with filters
parsnip entity list [--type <TYPE>] [--tag <TAG>] [--limit <N>]

# Get entity details
parsnip entity get <NAME>

# Add observation to existing entity
parsnip entity observe <NAME> -o "new fact"

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

# Traverse graph (BFS/DFS)
parsnip relation traverse <START> [-d <DEPTH>] [--direction outgoing|incoming|both]
parsnip relation traverse <START> --entity-types person --relation-types works_at

# Find shortest path
parsnip relation find-path <FROM> <TO> [--weighted] [--relation-types <TYPES>]
```

### Search Commands

```bash
# Basic search
parsnip search <QUERY>

# Search modes
parsnip search <QUERY> --mode exact      # Substring match
parsnip search <QUERY> --mode fuzzy      # Typo-tolerant
parsnip search <QUERY> --mode fulltext   # BM25 ranking
parsnip search <QUERY> --mode hybrid     # Fuzzy + fulltext

# Filter by tags
parsnip search --tag engineer --tag senior

# Cross-project search
parsnip search <QUERY> --all-projects

# With pagination
parsnip search <QUERY> --limit 20 --page 1
```

### Project Commands

```bash
# List all projects
parsnip project list

# Create project
parsnip project create <NAME> [-d "description"]

# Set default project
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

# Import to current project
parsnip import data.json

# Import to specific project
parsnip import data.json --target-project newproject

# Merge with existing data
parsnip import data.json --merge
```

### Server Commands

```bash
# Start MCP server (stdio)
parsnip serve

# Start MCP server (HTTP/SSE) - requires --features sse
parsnip serve -t sse --port 3000 --host 0.0.0.0
```

## MCP Integration

Parsnip includes a Model Context Protocol (MCP) server that gives AI assistants persistent memory.

### Claude Desktop Setup

Add to your `claude_desktop_config.json`:

```json
{
  "mcpServers": {
    "parsnip": {
      "command": "parsnip",
      "args": ["mcp"]
    }
  }
}
```

### Available MCP Tools

| Tool | Description |
|------|-------------|
| `search_knowledge` | Search entities with fuzzy/fulltext/hybrid modes |
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

## Search Modes

| Mode | Description | Use Case |
|------|-------------|----------|
| **Exact** | Substring matching | Precise queries, known names |
| **Fuzzy** | Nucleo-based, typo-tolerant | Misspellings, partial recall |
| **Full-text** | Tantivy BM25 ranking | Natural language queries |
| **Hybrid** | Fuzzy + full-text combined | Best overall recall |
| **Vector** | Cosine similarity (embeddings) | Semantic search |

### Fuzzy Search Configuration

```bash
# Adjust threshold (0.0 = match everything, 1.0 = exact only)
parsnip search "john smth" --mode fuzzy --threshold 0.3
```

## Storage Backends

### ReDB (Default)

Embedded key-value store with ACID transactions. Zero external dependencies.

```bash
# Data stored at:
# macOS: ~/Library/Application Support/parsnip/parsnip.redb
# Linux: ~/.local/share/parsnip/parsnip.redb
```

### SQLite

Relational backend compatible with SQL tools.

```bash
cargo install parsnip --features sqlite
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
| `PARSNIP_DATA_DIR` | Data directory path | Platform-specific |
| `PARSNIP_PROJECT` | Default project name | "default" |
| `PARSNIP_LOG` | Log level (trace/debug/info/warn/error) | "info" |

### Data Directory Locations

| Platform | Path |
|----------|------|
| macOS | `~/Library/Application Support/parsnip/` |
| Linux | `~/.local/share/parsnip/` |
| Windows | `%APPDATA%\parsnip\` |

## Performance

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
│   ├── parsnip-storage/    # Storage backends: ReDB, SQLite, Memory
│   ├── parsnip-search/     # Search engines: Exact, Fuzzy, FullText, Hybrid, Vector
│   ├── parsnip-cli/        # CLI binary with all commands
│   └── parsnip-mcp/        # MCP server with 12 tools
├── docs/
│   ├── spec.md             # Full specification
│   └── index.html          # Website
└── tests/                  # Integration tests
```

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
4. Run tests (`cargo test`)
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
- Lint with `cargo clippy`
- Test coverage target: >80%

## License

Dual-licensed under [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE) at your option.

---

**Built with Rust** · [Website](https://omar16100.github.io/parsnip/) · [GitHub](https://github.com/omar16100/parsnip)
