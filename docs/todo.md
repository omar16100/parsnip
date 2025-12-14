# Parsnip Development Todo

## Phase 1: Core Implementation

### parsnip-core
- [x] lib.rs - module exports
- [x] error.rs - error types
- [x] entity.rs - Entity type and operations
- [x] observation.rs - Observation handling
- [x] relation.rs - Relation type and operations
- [x] project.rs - Project/namespace management
- [x] graph.rs - Graph trait definitions
- [x] query.rs - Query builder
- [x] traversal.rs - Graph traversal (BFS, Dijkstra, filtered)

### parsnip-storage
- [x] lib.rs - module exports
- [x] traits.rs - Storage trait definitions
- [x] memory.rs - In-memory backend (testing)
- [x] redb.rs - ReDB backend (default)
- [x] sqlite.rs - SQLite backend (compat) - feature flag `sqlite`
- [x] migration.rs - Schema migrations

### parsnip-search
- [x] lib.rs - module exports
- [x] traits.rs - Search trait definitions
- [x] exact.rs - Exact substring search
- [x] fuzzy.rs - Fuzzy search (nucleo)
- [x] fulltext.rs - Full-text search (tantivy)
- [x] hybrid.rs - Combined search
- [x] vector.rs - Vector/semantic search (cosine similarity)

### parsnip-cli
- [x] main.rs - CLI entry point
- [x] config.rs - CLI configuration
- [x] output.rs - Output formatting
- [x] commands/mod.rs - Command modules
- [x] commands/entity.rs - Entity commands (fully connected)
- [x] commands/relation.rs - Relation commands (fully connected)
- [x] commands/search.rs - Search commands (fully connected)
- [x] commands/project.rs - Project commands (fully connected)
- [x] Connect CLI to storage backend
- [x] commands/io.rs - Import/export commands (JSON format)
- [x] commands/serve.rs - Start MCP server (wired to McpServer)

### parsnip-mcp
- [x] lib.rs - module exports
- [x] server.rs - Full MCP server with JSON-RPC handling
- [x] tools.rs - 12 tool definitions (search, CRUD, tags, traverse)
- [x] handlers.rs - Request/response types
- [x] transport.rs - stdio transport with JSON-RPC
- [x] sse.rs - SSE/HTTP transport with axum (feature flag `sse`)
- [x] Full MCP protocol implemented

## Changes Made

- Created all 5 crates with basic structure
- Implemented Entity, Observation, Relation, Project types
- Implemented SearchQuery with builder pattern
- Implemented Graph trait and KnowledgeGraph
- Implemented StorageBackend trait with MemoryStorage and RedbStorage
- Implemented SearchEngine trait with Exact, Fuzzy, FullText, Hybrid engines
- Created CLI with clap (entity, relation, search, project commands)
- Connected CLI commands to ReDB storage backend
- CLI fully functional: create/list/get/delete entities, relations, projects
- Search with filters (type, tag), graph traversal working
- Created MCP server skeleton
- All 25 tests passing
- Cross-project search verified working (--all-projects flag)
- Data persists in ReDB at ~/Library/Application Support/parsnip/parsnip.redb
- MCP server fully implemented with 11 tools
- MCP server tested: initialize, tools/list, tools/call all working
- Fixed tool schema to use camelCase (projectId, entityType, etc.) matching MCP protocol
- Serve command wired to MCP server (parsnip serve)
- Import/export commands implemented (JSON format)
- Export supports --all-projects for full backup
- Import supports --merge for incremental updates
- Pushed to GitHub: https://github.com/omar16100/parsnip
- Website: https://omar16100.github.io/parsnip/
- Added to Claude Code MCP: `claude mcp add parsnip`
- SQLite storage backend implemented with feature flag
- CLI supports `--features sqlite` or `--features redb` (default)
- Tantivy full-text search integrated to CLI and MCP
- CLI search supports `--mode fulltext` and `--mode hybrid`
- MCP search_knowledge supports `searchMode: "fulltext"` and `searchMode: "hybrid"`
- FullTextSearchEngine added to AppContext with feature flag
- Fulltext search index stored in ~/Library/Application Support/parsnip/index/
- SSE transport implemented with axum (feature flag `sse`)
- `parsnip serve -t sse --port 3000` starts HTTP server with SSE support
- SSE endpoints: /sse (events), /message (JSON-RPC), /health
- VectorSearchEngine implemented for semantic/embedding search (feature flag `vector`)
- SearchQuery supports `query_embedding` and `similarity_threshold` for vector search
- SearchMode::Vector added for cosine similarity based entity matching
- All 37 tests passing (4 config tests + 2 migration tests added)

### Security Fixes (Code Review)
- Export now writes files with 0o600 permissions (owner-only read/write)
- Fixed potential panic in entity.rs add_observation() - added expect() with clear message
- Fixed JSON serialization unwrap in server.rs - now returns proper error response

### CLI Features (v0.5.x)
- Config command: `parsnip config get|set|list|path|init`
- Config file at `~/.parsnip/config.toml` or `$XDG_CONFIG_HOME/parsnip/config.toml`
- Project use: `parsnip project use <name>` saves default_project to config
- Entity update: `parsnip entity update <name> --add-obs|--add-tag|--remove-tag|--set-type`
- Shell completions: `parsnip completions bash|zsh|fish|powershell|elvish`
- Export formats: `--format json|csv|graphml`
- Migration from knowledgegraph-mcp: `parsnip import --from-knowledgegraph <db.sqlite>`

### MCP Features (v0.5.x)
- `list_projects` tool: returns all projects with entity/relation counts

### Storage (v0.5.x)
- Schema migration framework with version tracking
- `Migratable` trait for storage backends
- `CURRENT_VERSION = 1` for initial schema

### Distribution (v0.6.x)
- Published all 5 crates to crates.io: parsnip-core, parsnip-storage, parsnip-search, parsnip-mcp, parsnip-cli
- `cargo install parsnip-cli` now works
- GitHub Actions CI: `.github/workflows/ci.yml` (test, clippy, format on push/PR)
- GitHub Actions Release: `.github/workflows/release.yml` (publish to crates.io on tag, build binaries)
- Universal shell installer: `install.sh` (curl | sh)
- Homebrew tap: `brew install omar16100/tap/parsnip`
- Website updated with tabbed install options (Shell, Cargo, Homebrew)

### Graph Traversal (v0.4.x)
- TraversalEngine with BFS and Dijkstra algorithms
- TraversalQuery builder pattern (start, target, max_depth, direction, filters)
- Path finding with weighted shortest path (Dijkstra) and unweighted (BFS)
- Filtered traversal by entity types and relation types
- CLI: `relation traverse` enhanced with `--relation-types`, `--entity-types` filters
- CLI: `relation find-path` command for path finding between entities
- MCP: `traverse_graph` tool for graph traversal via MCP protocol
- 6 unit tests for traversal algorithms (all passing)

## Installation

```bash
# Shell (Linux/macOS)
curl -fsSL https://raw.githubusercontent.com/omar16100/parsnip/main/install.sh | sh

# Cargo
cargo install parsnip-cli

# Homebrew (macOS)
brew install omar16100/tap/parsnip
```

## CLI Usage Examples

```bash
# Create a project
parsnip project create myproject -d "My knowledge graph"

# Add entities
parsnip -p myproject entity add John_Smith -t person -o "Works at Google" --tag engineer
parsnip -p myproject entity add Google -t company -o "Tech company"

# Add relations
parsnip -p myproject relation add John_Smith Google -t works_at

# Search
parsnip -p myproject search John
parsnip -p myproject search --tag engineer
parsnip -p myproject search "distributed systems" --mode fulltext
parsnip -p myproject search "machine learning" --mode hybrid

# Traverse graph
parsnip -p myproject relation traverse John_Smith -d 3
parsnip -p myproject relation traverse John_Smith -d 2 --direction outgoing
parsnip -p myproject relation traverse John_Smith --relation-types works_at,reports_to
parsnip -p myproject relation traverse John_Smith --entity-types person

# Find path between entities
parsnip -p myproject relation find-path Alice Carol
parsnip -p myproject relation find-path Alice Carol --weighted  # Dijkstra
parsnip -p myproject relation find-path Alice Carol -r reports_to  # Filter by relation

# Project stats
parsnip -p myproject project stats

# Export/Import
parsnip -p myproject export -o backup.json
parsnip export --all-projects -o full-backup.json
parsnip import backup.json --target-project newproject
parsnip import data.json --merge  # Add to existing data

# Start MCP server (stdio)
parsnip serve

# Start MCP server (SSE/HTTP) - requires --features sse
parsnip serve -t sse --port 3000

# Test SSE endpoints
curl http://localhost:3000/health
curl -X POST http://localhost:3000/message -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}'

# Configuration
parsnip config list                    # Show all config values
parsnip config get default_project     # Get a specific value
parsnip config set log_level debug     # Set a value
parsnip config path                    # Show config file path
parsnip config init                    # Create default config file

# Set default project
parsnip project use myproject          # Saves to config.toml

# Update entities
parsnip entity update John_Smith --add-obs "Promoted to senior"
parsnip entity update John_Smith --add-tag technical --remove-tag junior
parsnip entity update John_Smith --set-type senior_engineer

# Shell completions
parsnip completions bash >> ~/.bashrc
parsnip completions zsh >> ~/.zshrc
parsnip completions fish >> ~/.config/fish/completions/parsnip.fish

# Export formats
parsnip export -o backup.csv --format csv
parsnip export -o graph.graphml --format graphml

# Migrate from knowledgegraph-mcp (requires --features migrate)
parsnip import memory.db --from-knowledgegraph -p imported
```
