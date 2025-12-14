# Parsnip Product Vision & Technical Specification v0.1.0

```
╔══════════════════════════════════════════════════════════════════════════════╗
║                                                                              ║
║   ██████╗  █████╗ ██████╗ ███████╗███╗   ██╗██╗██████╗                       ║
║   ██╔══██╗██╔══██╗██╔══██╗██╔════╝████╗  ██║██║██╔══██╗                      ║
║   ██████╔╝███████║██████╔╝███████╗██╔██╗ ██║██║██████╔╝                      ║
║   ██╔═══╝ ██╔══██║██╔══██╗╚════██║██║╚██╗██║██║██╔═══╝                       ║
║   ██║     ██║  ██║██║  ██║███████║██║ ╚████║██║██║                           ║
║   ╚═╝     ╚═╝  ╚═╝╚═╝  ╚═╝╚══════╝╚═╝  ╚═══╝╚═╝╚═╝                           ║
║                                                                              ║
║   Memory Management Platform with Graph Database Support                     ║
║                                                                              ║
╚══════════════════════════════════════════════════════════════════════════════╝
```

## Document Status

This document is **vision-first**: it describes the intended user experience and
product guarantees for Parsnip. The appendices include technical sketches and
may lag behind the implementation.

## Table of Contents

- 1. Vision & Positioning
- 2. Target Users & Jobs-to-be-Done
- 3. Core Workflows
- 4. Product Guarantees & Success Metrics
- 5. Roadmap
- Appendix A: Architecture
- Appendix B: Data Model
- Appendix C: Repository & Crate Layout
- Appendix D: Dependencies
- Appendix E: CLI Reference (Target)
- Appendix F: MCP Tools (Target)
- Appendix G: Core Traits (Sketch)
- Appendix H: Storage Schema (Sketch)
- Appendix I: Performance Targets
- Appendix J: Configuration
- Appendix K: Build & Distribution
- Appendix L: Testing Strategy
- Appendix M: Migration Path
- Appendix N: License & Governance

## 1. Vision & Positioning

Parsnip is a **local-first memory graph** for AI assistants and knowledge
workers: capture durable facts as entities and relations, then reliably retrieve
them later with fast search and traversal.

### Problem

- Memory is often scattered across chats, notes, and one-off JSON/SQLite files.
- Retrieval needs to be fast, scoped (projects), and automatable (CLI/MCP).
- Users want strong defaults (offline, private, portable) without extra services.

### What Parsnip Is

- A single-binary graph store + search index with a stable, assistant-friendly model.
- A CLI for humans/scripts and an MCP server for assistant integrations.
- A “memory layer” you can adopt incrementally (import/migrate over time).

### Non-goals (v0.x)

- A hosted SaaS or collaborative multi-user service.
- A full note-taking UI (APIs/CLI/MCP first; UIs are optional).
- A general-purpose database (optimize for assistant memory primitives).

### Product Principles

- Local-first and offline by default.
- Cross-project recall without losing namespaces.
- Pipe-friendly, scriptable interfaces.
- Fast defaults; advanced features should be opt-in.

### Differentiators

- Graph-native model (entities/relations/observations), not “SQL + JSON blobs”.
- Cross-project search as a first-class feature.
- No external services required; portable on-disk format.

## 2. Target Users & Jobs-to-be-Done

### Target users

- **Assistant builders**: need persistent memory with predictable retrieval APIs.
- **Knowledge workers**: want a lightweight local “second brain” with structure.
- **Tooling/ops engineers**: want a single binary, simple deployment, clear data ownership.

### Jobs-to-be-done

- “When I learn a fact, capture it once and reuse it everywhere.”
- “When I ask a question later, retrieve the right entity quickly, even with typos.”
- “When context spans projects, search across them without mixing namespaces.”
- “When I switch machines, backup/migrate/restore without losing meaning.”

## 3. Core Workflows

### Capture (create/update)

Create entities with observations and tags, then add observations over time.

```bash
# Target CLI shape
parsnip entity add "John_Smith" --type person \
  --obs "Senior engineer at Google" \
  --tag mentor
parsnip entity observe "John_Smith" "Started new project on AI safety"
```

### Connect (relationships)

Create typed relations and traverse the graph from any entity.

```bash
parsnip relation add "John_Smith" "Google" --type works_at
parsnip relation traverse "John_Smith" --depth 2 --direction outgoing
```

### Recall (search + traversal)

Search should work within a project by default, and support explicit cross-project recall.

```bash
parsnip search "distributed systems"
parsnip search "jonh smth" --fuzzy --threshold 0.3
parsnip search "bail" --all-projects
```

### Integrate (MCP)

Run an MCP server over stdio so assistants can call tools like `search_knowledge`.

```bash
parsnip serve --transport stdio
```

### Operate (configuration, backup, migration)

- Default data lives under `~/.parsnip/` (config, DB, index).
- Import/export and migration should be verifiable and repeatable.

## 4. Product Guarantees & Success Metrics

### Guarantees (intended)

- Local-first by default; no network required for core operations.
- Durable storage with explicit project namespaces.
- Stable identifiers for entities/relations/observations once written.
- Compatibility guarantees tighten over time (see Appendix N).

### Success metrics (how we’ll know it’s working)

- **Time-to-first-result**: “find an entity I just created” in < 2 seconds end-to-end.
- **Retrieval quality**: top-3 contains the correct entity for common fuzzy queries.
- **Performance**: meet Appendix I targets on a published reference dataset/hardware profile.
- **Reliability**: import/migration is repeatable and verifiable.

## 5. Roadmap

### v0.1.x — MVP: fast local memory

- CRUD entities/relations/tags/observations
- Local storage (ReDB default) + SQLite compatibility mode
- CLI + MCP (stdio)
- Exact + fuzzy search; explicit cross-project recall

### v0.2.x — Search quality

- Full-text search (Tantivy) and hybrid ranking
- Query syntax and better filtering/sorting

### v0.3.x — Data management

- Import/export polish, backups, repair tooling
- Config ergonomics and migration hardening

### v0.4.x — Advanced (optional)

- Embeddings + semantic search (opt-in)
- Graph analytics and richer traversal

### v1.0 — Stability & ecosystem

- Stable API surface and compatibility guarantees
- Plugin surface and optional UIs

## Appendix A: Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                              USER INTERFACES                                 │
├─────────────────────────────┬───────────────────────────────────────────────┤
│         CLI (parsnip)       │              MCP Server                       │
│  • Interactive commands     │  • stdio transport                            │
│  • Shell completion         │  • SSE transport (future)                     │
│  • Pipe-friendly output     │  • Tool definitions                           │
└─────────────────────────────┴───────────────────────────────────────────────┘
                                      │
                                      ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                            PARSNIP-CORE                                      │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐              │
│  │  Graph Engine   │  │  Query Planner  │  │  Transaction    │              │
│  │  • Entities     │  │  • Optimization │  │  Manager        │              │
│  │  • Relations    │  │  • Traversal    │  │  • ACID         │              │
│  │  • Observations │  │  • Aggregation  │  │  • WAL          │              │
│  └─────────────────┘  └─────────────────┘  └─────────────────┘              │
└─────────────────────────────────────────────────────────────────────────────┘
                                      │
                                      ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                            PARSNIP-SEARCH                                    │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐              │
│  │  Fuzzy Search   │  │  Full-Text      │  │  Vector Search  │              │
│  │  (nucleo)       │  │  (tantivy)      │  │  (optional)     │              │
│  └─────────────────┘  └─────────────────┘  └─────────────────┘              │
└─────────────────────────────────────────────────────────────────────────────┘
                                      │
                                      ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                           PARSNIP-STORAGE                                    │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐              │
│  │  ReDB Backend   │  │  SQLite Backend │  │  Remote Backend │              │
│  │  (default)      │  │  (compat mode)  │  │  (future)       │              │
│  └─────────────────┘  └─────────────────┘  └─────────────────┘              │
└─────────────────────────────────────────────────────────────────────────────┘
                                      │
                                      ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                              FILE SYSTEM                                     │
│  ~/.parsnip/                                                                 │
│  ├── data/                    # Graph data (ReDB)                           │
│  │   └── parsnip.db                                                         │
│  ├── index/                   # Tantivy search index                        │
│  ├── config.toml              # Configuration                               │
│  └── parsnip.log              # Logs (optional)                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

## Appendix B: Data Model

### Entity (Node)

| Field        | Type                | Description                      |
|--------------|---------------------|----------------------------------|
| id           | ULID                | Unique identifier (sortable)     |
| project_id   | ULID                | Project namespace                |
| name         | String (indexed)    | Entity name (unique per project) |
| entity_type  | String (indexed)    | Category (person, project, etc.) |
| observations | Vec<Observation>    | Facts about the entity           |
| tags         | Vec<String>         | Categorical labels               |
| metadata     | HashMap<String,Val> | Arbitrary key-value pairs        |
| created_at   | DateTime<Utc>       | Creation timestamp               |
| updated_at   | DateTime<Utc>       | Last modification timestamp      |
| embedding    | Option<Vec<f32>>    | Vector embedding (optional)      |

### Observation (Embedded in Entity)

| Field      | Type            | Description                 |
|------------|-----------------|------------------------------|
| id         | ULID            | Unique identifier            |
| content    | String (FTS)    | The observation text         |
| source     | Option<String>  | Where this info came from    |
| confidence | Option<f32>     | Confidence score (0.0-1.0)   |
| created_at | DateTime<Utc>   | When observed                |

### Relation (Edge)

| Field         | Type                | Description                      |
|---------------|---------------------|----------------------------------|
| id            | ULID                | Unique identifier                |
| project_id    | ULID                | Project namespace                |
| from_id       | ULID (indexed)      | Source entity                    |
| to_id         | ULID (indexed)      | Target entity                    |
| relation_type | String (indexed)    | Relationship type (works_at)     |
| weight        | Option<f64>         | Relationship strength            |
| metadata      | HashMap<String,Val> | Arbitrary key-value pairs        |
| created_at    | DateTime<Utc>       | Creation timestamp               |

### Project (Namespace)

| Field       | Type            | Description                 |
|-------------|-----------------|------------------------------|
| id          | ULID            | Unique identifier            |
| name        | String (unique) | Project slug (alphanumeric)  |
| description | Option<String>  | Human-readable description   |
| created_at  | DateTime<Utc>   | Creation timestamp           |
| settings    | ProjectSettings | Project-specific config      |

## Appendix C: Repository & Crate Layout

> Note: this section describes the intended end-state layout. Some directories or
> crates may be empty while the workspace is being bootstrapped.

```
parsnip/
├── Cargo.toml                      # Workspace definition
├── LICENSE                         # MIT OR Apache-2.0
├── README.md
├── CHANGELOG.md
│
├── crates/
│   │
│   ├── parsnip-core/               # Core graph logic
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── entity.rs           # Entity type and operations
│   │       ├── relation.rs         # Relation type and operations
│   │       ├── observation.rs      # Observation handling
│   │       ├── project.rs          # Project/namespace management
│   │       ├── graph.rs            # Graph trait and implementations
│   │       ├── query.rs            # Query builder and execution
│   │       ├── transaction.rs      # Transaction management
│   │       └── error.rs            # Error types
│   │
│   ├── parsnip-storage/            # Storage backends
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── traits.rs           # Storage trait definitions
│   │       ├── redb.rs             # ReDB backend (default)
│   │       ├── sqlite.rs           # SQLite backend (compat)
│   │       ├── memory.rs           # In-memory backend (testing)
│   │       └── migration.rs        # Schema migrations
│   │
│   ├── parsnip-search/             # Search engines
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── traits.rs           # Search trait definitions
│   │       ├── fuzzy.rs            # Fuzzy search (nucleo)
│   │       ├── fulltext.rs         # Full-text search (tantivy)
│   │       ├── vector.rs           # Vector search (optional)
│   │       └── hybrid.rs           # Combined search strategies
│   │
│   ├── parsnip-cli/                # CLI binary
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── main.rs
│   │       ├── commands/
│   │       │   ├── mod.rs
│   │       │   ├── entity.rs       # Entity CRUD commands
│   │       │   ├── relation.rs     # Relation commands
│   │       │   ├── search.rs       # Search commands
│   │       │   ├── project.rs      # Project management
│   │       │   ├── import.rs       # Import from various formats
│   │       │   ├── export.rs       # Export to various formats
│   │       │   └── serve.rs        # Start MCP server
│   │       ├── output.rs           # Output formatting (table, json)
│   │       └── config.rs           # CLI configuration
│   │
│   └── parsnip-mcp/                # MCP server
│       ├── Cargo.toml
│       └── src/
│           ├── lib.rs
│           ├── server.rs           # MCP server implementation
│           ├── tools.rs            # Tool definitions
│           ├── handlers.rs         # Tool handlers
│           └── transport.rs        # stdio/SSE transports
│
├── tests/                          # Integration tests
│   ├── cli_tests.rs
│   ├── mcp_tests.rs
│   └── search_tests.rs
│
├── benches/                        # Benchmarks
│   ├── search_bench.rs
│   └── storage_bench.rs
│
└── assets/
    └── completions/                # Shell completions
        ├── parsnip.bash
        ├── parsnip.zsh
        ├── parsnip.fish
        └── _parsnip.ps1
```

## Appendix D: Dependencies

```toml
[workspace.dependencies]
# Async Runtime
tokio = { version = "1.40", features = ["full"] }

# CLI
clap = { version = "4.5", features = ["derive", "env", "wrap_help"] }
clap_complete = "4.5"

# Serialization
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
toml = "0.8"

# Storage
redb = "2.2"
rusqlite = { version = "0.32", features = ["bundled"] }

# Search
tantivy = "0.22"
nucleo = "0.5"
nucleo-matcher = "0.3"

# IDs
ulid = { version = "1.1", features = ["serde"] }

# Time
chrono = { version = "0.4", features = ["serde"] }

# Error Handling
thiserror = "1.0"
anyhow = "1.0"

# Logging
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }

# Async
async-trait = "0.1"
futures = "0.3"

# Testing
tempfile = "3.13"
assert_cmd = "2.0"
predicates = "3.1"
```

## Appendix E: CLI Reference (Target)

```
USAGE: parsnip [OPTIONS] <COMMAND>

COMMANDS:
  entity         Create, read, update, delete entities
  relation       Manage relationships between entities
  search         Search across the knowledge graph
  project        Manage projects/namespaces
  import         Import data from external sources
  export         Export data to various formats
  serve          Start MCP server
  config         Manage configuration
  completions    Generate shell completions

GLOBAL OPTIONS:
  -p, --project <NAME>    Project namespace [default: default]
  -d, --data-dir <PATH>   Data directory [default: ~/.parsnip]
  -f, --format <FMT>      Output format: table, json, csv [default: table]
  -v, --verbose           Increase verbosity (-v, -vv, -vvv)
  -q, --quiet             Suppress output except errors
  -h, --help              Print help
  -V, --version           Print version
```

### Entity Commands

```bash
# Create entity
parsnip entity add "John_Smith" \
  --type person \
  --obs "Senior engineer at Google" \
  --obs "Expert in distributed systems" \
  --tag technical \
  --tag mentor

# List entities
parsnip entity list [--type <TYPE>] [--tag <TAG>] [--limit <N>]

# Get entity details
parsnip entity get "John_Smith"

# Update entity
parsnip entity update "John_Smith" \
  --add-obs "Promoted to Staff Engineer in 2024" \
  --add-tag promoted

# Delete entity
parsnip entity delete "John_Smith" [--force]

# Add observation
parsnip entity observe "John_Smith" "Started new project on AI safety"
```

### Relation Commands

```bash
# Create relation
parsnip relation add "John_Smith" "Google" --type works_at
parsnip relation add "John_Smith" "Jane_Doe" --type mentors --weight 0.8

# List relations
parsnip relation list [--from <ENTITY>] [--to <ENTITY>] [--type <TYPE>]

# Delete relation
parsnip relation delete "John_Smith" "Google" --type works_at

# Traverse graph
parsnip relation traverse "John_Smith" --depth 2 --direction outgoing
```

### Search Commands

```bash
# Basic search (current project)
parsnip search "distributed systems"

# Cross-project search (KEY FEATURE)
parsnip search "bail" --all-projects

# Fuzzy search
parsnip search "jonh smth" --fuzzy --threshold 0.3

# Search with filters
parsnip search "engineer" \
  --type person \
  --tag technical \
  --limit 10

# Search by tag only
parsnip search --tag urgent --tag critical

# Full-text search
parsnip search "expert in machine learning" --mode fulltext

# Output with relations
parsnip search "John" --include-relations
```

### Project Commands

```bash
# List projects
parsnip project list

# Create project
parsnip project create "security-research" --description "Security findings"

# Switch default project
parsnip project use "security-research"

# Delete project
parsnip project delete "old-project" --force

# Project stats
parsnip project stats [PROJECT]
```

### Import/Export

```bash
# Export
parsnip export --format json > backup.json
parsnip export --format csv --output entities.csv
parsnip export --format graphml --output graph.graphml

# Import
parsnip import backup.json
parsnip import --format csv entities.csv
parsnip import --from-knowledgegraph ~/.knowledge-graph/knowledgegraph.db
```

### MCP Server

```bash
# Start MCP server (stdio)
parsnip serve

# Start with specific project
parsnip serve --project security-research

# Start with all-projects search enabled by default
parsnip serve --allow-cross-project
```

## Appendix F: MCP Tools (Target)

| Tool                | Description                              |
|---------------------|------------------------------------------|
| search_knowledge    | Search entities by text, tags, or type   |
| create_entities     | Create new entities with observations    |
| add_observations    | Add facts to existing entities           |
| create_relations    | Create relationships between entities    |
| delete_entities     | Remove entities and their relations      |
| delete_observations | Remove specific observations             |
| delete_relations    | Remove specific relationships            |
| read_graph          | Get full graph for a project             |
| open_nodes          | Get specific entities by name            |
| add_tags            | Add categorical tags to entities         |
| remove_tags         | Remove tags from entities                |
| list_projects       | List all available projects              |
| traverse_graph      | Graph traversal queries                  |

### search_knowledge Schema (Enhanced)

```json
{
  "name": "search_knowledge",
  "description": "Search entities across knowledge graph",
  "inputSchema": {
    "type": "object",
    "properties": {
      "query": {
        "type": "string",
        "description": "Search text"
      },
      "search_all": {
        "type": "boolean",
        "description": "Search ALL projects",
        "default": false
      },
      "project_id": {
        "type": "string",
        "description": "Project to search (ignored if search_all=true)"
      },
      "search_mode": {
        "type": "string",
        "enum": ["exact", "fuzzy", "fulltext", "hybrid"],
        "default": "exact"
      },
      "fuzzy_threshold": {
        "type": "number",
        "minimum": 0.0,
        "maximum": 1.0,
        "default": 0.3
      },
      "entity_types": {
        "type": "array",
        "items": { "type": "string" }
      },
      "tags": {
        "type": "array",
        "items": { "type": "string" }
      },
      "tag_match_mode": {
        "type": "string",
        "enum": ["any", "all"],
        "default": "any"
      },
      "include_relations": {
        "type": "boolean",
        "default": true
      },
      "page": {
        "type": "integer",
        "minimum": 0,
        "default": 0
      },
      "page_size": {
        "type": "integer",
        "minimum": 1,
        "maximum": 1000,
        "default": 100
      }
    }
  }
}
```

## Appendix G: Core Traits (Sketch)

```rust
// Source of truth: crates/parsnip-core/src/graph.rs
#[async_trait]
pub trait KnowledgeGraph: Send + Sync {
    // Entity operations
    async fn create_entity(&self, entity: NewEntity, project: &ProjectId) -> Result<Entity>;
    async fn get_entity(&self, name: &str, project: &ProjectId) -> Result<Option<Entity>>;
    async fn get_entities(&self, names: &[String], project: &ProjectId) -> Result<Vec<Entity>>;
    async fn update_entity(&self, entity: &Entity) -> Result<Entity>;
    async fn delete_entity(&self, name: &str, project: &ProjectId) -> Result<()>;

    // Observation operations
    async fn add_observations(
        &self,
        name: &str,
        observations: Vec<String>,
        project: &ProjectId,
    ) -> Result<Entity>;
    async fn remove_observations(
        &self,
        name: &str,
        observation_ids: &[String],
        project: &ProjectId,
    ) -> Result<Entity>;

    // Tag operations
    async fn add_tags(&self, name: &str, tags: Vec<String>, project: &ProjectId) -> Result<Entity>;
    async fn remove_tags(&self, name: &str, tags: &[String], project: &ProjectId) -> Result<Entity>;

    // Relation operations
    async fn create_relation(&self, relation: NewRelation, project: &ProjectId) -> Result<Relation>;
    async fn get_relations(
        &self,
        entity_name: &str,
        direction: Direction,
        project: &ProjectId,
    ) -> Result<Vec<Relation>>;
    async fn delete_relation(
        &self,
        from: &str,
        to: &str,
        relation_type: &str,
        project: &ProjectId,
    ) -> Result<()>;

    // Graph operations
    async fn read_graph(&self, project: &ProjectId) -> Result<Graph>;
    async fn traverse(
        &self,
        start: &str,
        depth: u32,
        direction: Direction,
        project: &ProjectId,
    ) -> Result<Graph>;

    // Search operations
    async fn search(&self, query: SearchQuery) -> Result<PaginatedResults<Entity>>;

    // Project operations
    async fn list_projects(&self) -> Result<Vec<Project>>;
    async fn create_project(&self, name: &str, description: Option<&str>) -> Result<Project>;
    async fn get_project(&self, name: &str) -> Result<Option<Project>>;
    async fn get_project_by_id(&self, id: &ProjectId) -> Result<Option<Project>>;
    async fn delete_project(&self, name: &str) -> Result<()>;
    async fn get_or_create_default_project(&self) -> Result<Project>;
}

/// Search query builder
pub struct SearchQuery {
    pub text: Option<String>,
    pub mode: SearchMode,
    pub fuzzy_threshold: f32,
    pub entity_types: Vec<String>,
    pub tags: Vec<String>,
    pub tag_match_mode: TagMatchMode,
    pub projects: ProjectScope,
    pub pagination: Pagination,
    pub include_relations: bool,
}

pub enum ProjectScope {
    Single(ProjectId),
    Multiple(Vec<ProjectId>),
    All,
}

pub enum SearchMode {
    Exact,
    Fuzzy,
    FullText,
    Hybrid,  // Combines fuzzy + fulltext
}

pub enum TagMatchMode {
    Any,
    All,
}

pub struct Pagination {
    pub page: usize,
    pub page_size: usize,
}
```

## Appendix H: Storage Schema (Sketch)

```rust
// ReDB table definitions

// Entities table: (project_id, name) -> Entity
const ENTITIES: TableDefinition<(&str, &str), &[u8]> =
    TableDefinition::new("entities");

// Entity index by ID: entity_id -> (project_id, name)
const ENTITY_INDEX: TableDefinition<&str, (&str, &str)> =
    TableDefinition::new("entity_index");

// Relations table: relation_id -> Relation
const RELATIONS: TableDefinition<&str, &[u8]> =
    TableDefinition::new("relations");

// Relation index: (project_id, from_id) -> Vec<relation_id>
const RELATION_FROM_INDEX: MultimapTableDefinition<(&str, &str), &str> =
    MultimapTableDefinition::new("relation_from_idx");

// Relation index: (project_id, to_id) -> Vec<relation_id>
const RELATION_TO_INDEX: MultimapTableDefinition<(&str, &str), &str> =
    MultimapTableDefinition::new("relation_to_idx");

// Projects table: project_id -> Project
const PROJECTS: TableDefinition<&str, &[u8]> =
    TableDefinition::new("projects");

// Project name index: name -> project_id
const PROJECT_NAME_INDEX: TableDefinition<&str, &str> =
    TableDefinition::new("project_name_idx");

// Tags index: (project_id, tag) -> Vec<entity_id>
const TAG_INDEX: MultimapTableDefinition<(&str, &str), &str> =
    MultimapTableDefinition::new("tag_idx");

// Entity type index: (project_id, type) -> Vec<entity_id>
const TYPE_INDEX: MultimapTableDefinition<(&str, &str), &str> =
    MultimapTableDefinition::new("type_idx");
```

## Appendix I: Performance Targets

| Metric                       | Target   | Notes                     |
|------------------------------|----------|---------------------------|
| Cold start                   | < 10ms   | Native binary, no VM      |
| MCP server startup           | < 50ms   | Including index load      |
| Entity create                | < 1ms    | Single entity             |
| Entity batch create (100)    | < 10ms   | Transactional             |
| Exact search (10k entities)  | < 5ms    | Index lookup              |
| Fuzzy search (10k entities)  | < 20ms   | Nucleo                    |
| FTS search (10k entities)    | < 10ms   | Tantivy                   |
| Cross-project search (100k)  | < 100ms  | All projects              |
| Graph traversal (depth 3)    | < 50ms   | BFS/DFS                   |
| Memory (idle)                | < 20MB   | Base memory               |
| Memory (10k entities loaded) | < 100MB  | With search index         |
| Binary size                  | < 15MB   | Release build, stripped   |
| Database size (10k entities) | < 50MB   | With FTS index            |

## Appendix J: Configuration

```toml
# ~/.parsnip/config.toml

[general]
default_project = "default"
data_dir = "~/.parsnip/data"
log_level = "info"                  # trace, debug, info, warn, error

[storage]
backend = "redb"                    # redb, sqlite
# For SQLite compatibility mode:
# backend = "sqlite"
# sqlite_path = "~/.parsnip/data/parsnip.db"

[search]
fuzzy_threshold = 0.3               # Default fuzzy match threshold
fulltext_enabled = true             # Enable Tantivy FTS
index_dir = "~/.parsnip/index"

[mcp]
transport = "stdio"                 # stdio, sse (future)
allow_cross_project = true          # Allow search_all by default
default_page_size = 100

[cli]
output_format = "table"             # table, json, csv
color = "auto"                      # auto, always, never
pager = true                        # Use pager for long output
```

## Appendix K: Build & Distribution

### Build Targets

| Target                       | Notes                        |
|------------------------------|------------------------------|
| x86_64-unknown-linux-gnu     | Linux AMD64 (glibc)          |
| x86_64-unknown-linux-musl    | Linux AMD64 (static, Alpine) |
| aarch64-unknown-linux-gnu    | Linux ARM64 (Raspberry Pi)   |
| x86_64-apple-darwin          | macOS Intel                  |
| aarch64-apple-darwin         | macOS Apple Silicon          |
| x86_64-pc-windows-msvc       | Windows AMD64                |

### Distribution Channels

- GitHub Releases (binaries + checksums)
- Homebrew: `brew install parsnip`
- Cargo: `cargo install parsnip`
- Docker: `ghcr.io/parsnip-ai/parsnip:latest`
- Nix: `nix run github:parsnip-ai/parsnip`

## Appendix L: Testing Strategy

- **Unit Tests**: Core graph operations, search algorithms, serialization
- **Integration Tests**: End-to-end CLI workflows, MCP protocol compliance
- **Property-Based Tests**: Graph invariants, search consistency
- **Benchmarks**: Search performance at scale, write throughput

**Coverage Target**: > 80%

## Appendix M: Migration Path

```bash
# One-command migration from knowledgegraph-mcp
parsnip import --from-knowledgegraph ~/.knowledge-graph/knowledgegraph.db

# This will:
# 1. Read SQLite database
# 2. Convert entities/relations to Parsnip format
# 3. Preserve project namespaces
# 4. Build search index

# Verify migration
parsnip project list
parsnip search "bail" --all-projects
```

## Appendix N: License & Governance

**License**: MIT OR Apache-2.0 (dual license, user's choice)

**Repository**: github.com/parsnip-ai/parsnip

**Governance**:
- Open to contributions via PRs
- RFC process for major changes
- Semantic versioning
- Backward compatibility commitment after v1.0
