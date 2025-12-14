//! Parsnip CLI - Command line interface for the knowledge graph

use std::path::PathBuf;
use std::sync::Arc;

use clap::{Args, Parser, Subcommand};
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

mod commands;
mod config;
mod output;

use commands::{entity, io, project, relation, search};
use parsnip_mcp::McpServer;

#[cfg(feature = "redb")]
use parsnip_storage::RedbStorage;

#[cfg(feature = "sqlite")]
use parsnip_storage::SqliteStorage;

#[cfg(feature = "fulltext")]
use parsnip_search::FullTextSearchEngine;

#[derive(Parser)]
#[command(name = "parsnip")]
#[command(author, version, about = "Memory management platform for AI assistants")]
pub struct Cli {
    /// Project namespace
    #[arg(short, long, default_value = "default", global = true)]
    pub project: String,

    /// Data directory
    #[arg(short, long, global = true)]
    pub data_dir: Option<String>,

    /// Output format: table, json, csv
    #[arg(short, long, default_value = "table", global = true)]
    pub format: String,

    /// Verbosity level (-v, -vv, -vvv)
    #[arg(short, long, action = clap::ArgAction::Count, global = true)]
    pub verbose: u8,

    /// Suppress output except errors
    #[arg(short, long, global = true)]
    pub quiet: bool,

    #[command(subcommand)]
    pub command: Commands,
}

impl Cli {
    /// Get the data directory path
    pub fn data_dir(&self) -> PathBuf {
        self.data_dir
            .as_ref()
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                dirs::data_dir()
                    .unwrap_or_else(|| PathBuf::from("."))
                    .join("parsnip")
            })
    }
}

#[derive(Subcommand)]
pub enum Commands {
    /// Manage entities
    Entity(entity::EntityArgs),
    /// Manage relations
    Relation(relation::RelationArgs),
    /// Search the knowledge graph
    Search(search::SearchArgs),
    /// Manage projects
    Project(project::ProjectArgs),
    /// Import data from JSON file
    Import(io::ImportArgs),
    /// Export data to JSON file
    Export(io::ExportArgs),
    /// Start MCP server
    Serve(ServeArgs),
}

/// Arguments for the serve command
#[derive(Args)]
pub struct ServeArgs {
    /// Transport type: stdio or sse
    #[arg(short, long, default_value = "stdio")]
    pub transport: String,

    /// Port for SSE transport (default: 3000)
    #[arg(long, default_value = "3000")]
    pub port: u16,

    /// Host to bind for SSE transport
    #[arg(long, default_value = "127.0.0.1")]
    pub host: String,
}

// Storage type alias based on feature
#[cfg(feature = "redb")]
pub type Storage = RedbStorage;

#[cfg(all(feature = "sqlite", not(feature = "redb")))]
pub type Storage = SqliteStorage;

/// Application context with storage and search backends
pub struct AppContext {
    pub storage: Arc<Storage>,
    #[cfg(feature = "fulltext")]
    pub fulltext: Option<Arc<FullTextSearchEngine>>,
}

impl AppContext {
    pub async fn new(cli: &Cli) -> anyhow::Result<Self> {
        let data_dir = cli.data_dir();
        std::fs::create_dir_all(&data_dir)?;

        #[cfg(feature = "redb")]
        let storage = {
            let db_path = data_dir.join("parsnip.redb");
            tracing::debug!("Using ReDB database at: {:?}", db_path);
            RedbStorage::open(&db_path)?
        };

        #[cfg(all(feature = "sqlite", not(feature = "redb")))]
        let storage = {
            let db_path = data_dir.join("parsnip.sqlite");
            tracing::debug!("Using SQLite database at: {:?}", db_path);
            SqliteStorage::open(&db_path)?
        };

        // Initialize full-text search index
        #[cfg(feature = "fulltext")]
        let fulltext = {
            let index_path = data_dir.join("index");
            std::fs::create_dir_all(&index_path)?;
            match FullTextSearchEngine::new(&index_path) {
                Ok(engine) => {
                    tracing::debug!("Full-text search index at: {:?}", index_path);
                    Some(Arc::new(engine))
                }
                Err(e) => {
                    tracing::warn!("Failed to initialize full-text search: {}", e);
                    None
                }
            }
        };

        Ok(Self {
            storage: Arc::new(storage),
            #[cfg(feature = "fulltext")]
            fulltext,
        })
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Set up logging based on verbosity
    let filter = match cli.verbose {
        0 if cli.quiet => "error",
        0 => "warn",
        1 => "info",
        2 => "debug",
        _ => "trace",
    };

    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| filter.into()))
        .init();

    tracing::debug!("Starting parsnip CLI");

    // Initialize storage
    let ctx = AppContext::new(&cli).await?;

    match &cli.command {
        Commands::Entity(args) => entity::run(args, &cli, &ctx).await?,
        Commands::Relation(args) => relation::run(args, &cli, &ctx).await?,
        Commands::Search(args) => search::run(args, &cli, &ctx).await?,
        Commands::Project(args) => project::run(args, &cli, &ctx).await?,
        Commands::Import(args) => io::run_import(args, &cli, &ctx).await?,
        Commands::Export(args) => io::run_export(args, &cli, &ctx).await?,
        Commands::Serve(args) => {
            let server = Arc::new(McpServer::new(ctx.storage.clone()));
            match args.transport.as_str() {
                #[cfg(feature = "sse")]
                "sse" | "http" => {
                    let addr = format!("{}:{}", args.host, args.port);
                    tracing::info!("Starting MCP server with SSE transport on {}", addr);
                    parsnip_mcp::run_sse_server(server, &addr).await?;
                }
                #[cfg(not(feature = "sse"))]
                "sse" | "http" => {
                    anyhow::bail!("SSE transport not available. Rebuild with --features sse");
                }
                _ => {
                    tracing::info!("Starting MCP server on stdio...");
                    server.run_stdio().await?;
                }
            }
        }
    }

    Ok(())
}
