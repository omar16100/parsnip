//! Parsnip CLI - Command line interface for the knowledge graph

use std::path::PathBuf;
use std::sync::Arc;

use clap::{Parser, Subcommand};
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

mod commands;
mod config;
mod output;

use commands::{entity, project, relation, search};
use parsnip_mcp::McpServer;
use parsnip_storage::RedbStorage;

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
    /// Start MCP server
    Serve,
}

/// Application context with storage backend
pub struct AppContext {
    pub storage: Arc<RedbStorage>,
}

impl AppContext {
    pub async fn new(cli: &Cli) -> anyhow::Result<Self> {
        let data_dir = cli.data_dir();
        std::fs::create_dir_all(&data_dir)?;

        let db_path = data_dir.join("parsnip.redb");
        tracing::debug!("Using database at: {:?}", db_path);

        let storage = RedbStorage::open(&db_path)?;

        Ok(Self {
            storage: Arc::new(storage),
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
        Commands::Serve => {
            tracing::info!("Starting MCP server on stdio...");
            let server = McpServer::new(ctx.storage.clone());
            server.run_stdio().await?;
        }
    }

    Ok(())
}
