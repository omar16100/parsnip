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

### parsnip-storage
- [x] lib.rs - module exports
- [x] traits.rs - Storage trait definitions
- [x] memory.rs - In-memory backend (testing)
- [x] redb.rs - ReDB backend (default)
- [ ] sqlite.rs - SQLite backend (compat)
- [ ] migration.rs - Schema migrations

### parsnip-search
- [x] lib.rs - module exports
- [x] traits.rs - Search trait definitions
- [x] exact.rs - Exact substring search
- [x] fuzzy.rs - Fuzzy search (nucleo)
- [x] fulltext.rs - Full-text search (tantivy)
- [x] hybrid.rs - Combined search

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
- [ ] commands/import.rs
- [ ] commands/export.rs
- [x] commands/serve.rs - Start MCP server (wired to McpServer)

### parsnip-mcp
- [x] lib.rs - module exports
- [x] server.rs - Full MCP server with JSON-RPC handling
- [x] tools.rs - 11 tool definitions (search, CRUD, tags)
- [x] handlers.rs - Request/response types
- [x] transport.rs - stdio transport with JSON-RPC
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
- All 23 tests passing
- Cross-project search verified working (--all-projects flag)
- Data persists in ReDB at ~/Library/Application Support/parsnip/parsnip.redb
- MCP server fully implemented with 11 tools
- MCP server tested: initialize, tools/list, tools/call all working
- Serve command wired to MCP server (parsnip serve)
- Pushed to GitHub: https://github.com/omar16100/parsnip
- Added to Claude Code MCP: `claude mcp add parsnip`

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

# Traverse graph
parsnip -p myproject relation traverse John_Smith -d 3

# Project stats
parsnip -p myproject project stats
```
