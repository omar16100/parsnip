//! Parsnip CLI - Command line interface for the knowledge graph

use std::path::{Path, PathBuf};
use std::sync::Arc;

use clap::{Args, Parser, Subcommand};
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

mod commands;
mod config;
mod view;

use commands::{completions, config as config_cmd, entity, io, project, relation, search};
use parsnip_mcp::McpServer;
use parsnip_storage::StorageBackend;

#[cfg(feature = "redb")]
use parsnip_storage::RedbStorage;

#[cfg(all(feature = "sqlite", not(feature = "redb")))]
use parsnip_storage::SqliteStorage;

#[cfg(feature = "remote")]
use parsnip_storage::remote::client::RemoteStorage;

#[derive(Parser)]
#[command(name = "parsnip")]
#[command(
    author,
    version,
    about = "Memory management platform for AI assistants"
)]
pub struct Cli {
    /// Project namespace
    #[arg(short, long, default_value = "default", global = true)]
    pub project: String,

    /// Data directory
    #[arg(short, long, global = true)]
    pub data_dir: Option<String>,

    // One global flag rather than one per command. `export` used to declare its own
    // `--format` of a different type under the same clap id, which made `parsnip export`
    // panic at parse time in the released 0.1.0.
    /// Output format: table, json, csv (graphml is export only)
    #[arg(short, long, value_enum, default_value_t = view::OutputFormat::Table, global = true)]
    pub format: view::OutputFormat,

    /// Verbosity level (-v, -vv, -vvv)
    #[arg(short, long, action = clap::ArgAction::Count, global = true)]
    pub verbose: u8,

    /// Suppress output except errors
    #[arg(short, long, global = true)]
    pub quiet: bool,

    /// Talk to a parsnip daemon over HTTP instead of opening the local database,
    /// e.g. http://100.64.0.1:8787. Also settable via `parsnip config set server_url`.
    #[arg(long, env = "PARSNIP_SERVER", global = true)]
    pub server: Option<String>,

    // Deliberately not `conflicts_with = "server"`: clap counts a value taken from
    // PARSNIP_SERVER as the flag being present, so with the variable exported (the normal
    // remote-mode setup) `--local` was rejected outright instead of overriding it.
    /// Ignore any configured server and use the local database for this invocation.
    /// Wins over --server, PARSNIP_SERVER and server_url.
    #[arg(long, global = true)]
    pub local: bool,

    // hide_env_values: clap otherwise prints the variable's value in --help, which would
    // put the token into any pasted help output.
    /// Bearer token: required by `serve` for non-localhost, sent by remote-mode clients
    #[arg(
        long,
        env = "PARSNIP_AUTH_TOKEN",
        global = true,
        hide_env_values = true
    )]
    pub auth_token: Option<String>,

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
    /// Manage configuration
    Config(config_cmd::ConfigArgs),
    /// Generate shell completions
    Completions(completions::CompletionsArgs),
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

    // NOTE: --auth-token lives on `Cli` as a global arg (shared with remote client mode).
    // Declaring it here as well would duplicate the clap arg ID.
    /// Allow binding to non-localhost addresses (requires --auth-token)
    #[arg(long)]
    pub allow_remote: bool,
}

/// Storage is chosen at runtime, not at compile time: the same binary either opens the
/// local database or talks to a daemon over HTTP. Command handlers call through the
/// trait either way, so they do not care which.
pub type Storage = dyn StorageBackend;

/// Application context with storage and search backends
pub struct AppContext {
    pub storage: Arc<Storage>,

    /// Present only in remote mode. Kept alongside `storage` so `search` can reach the
    /// server-side search RPC without downcasting the trait object.
    #[cfg(feature = "remote")]
    pub remote: Option<Arc<RemoteStorage>>,
}

/// Create directory with secure permissions (0700 on Unix)
fn create_secure_dir(path: &Path) -> std::io::Result<()> {
    if path.exists() {
        return Ok(());
    }

    // Create parent directories first
    if let Some(parent) = path.parent() {
        create_secure_dir(parent)?;
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        std::fs::DirBuilder::new().mode(0o700).create(path)
    }

    #[cfg(not(unix))]
    {
        std::fs::create_dir(path)
    }
}

impl AppContext {
    pub async fn new(cli: &Cli, server_url: Option<&str>) -> anyhow::Result<Self> {
        #[cfg(feature = "remote")]
        if let Some(url) = server_url {
            let remote = Arc::new(RemoteStorage::connect(url, cli.auth_token.clone()).await?);
            tracing::debug!(url, "using remote storage");
            return Ok(Self {
                storage: remote.clone(),
                remote: Some(remote),
            });
        }
        #[cfg(not(feature = "remote"))]
        if server_url.is_some() {
            anyhow::bail!("--server needs a build with the `remote` feature");
        }

        let data_dir = cli.data_dir();
        create_secure_dir(&data_dir)?;

        #[cfg(feature = "redb")]
        let storage = {
            let db_path = data_dir.join("parsnip.redb");
            tracing::debug!("Using ReDB database at: {:?}", db_path);
            RedbStorage::open(&db_path).map_err(|e| explain_lock_error(e, &db_path))?
        };

        #[cfg(all(feature = "sqlite", not(feature = "redb")))]
        let storage = {
            let db_path = data_dir.join("parsnip.sqlite");
            tracing::debug!("Using SQLite database at: {:?}", db_path);
            SqliteStorage::open(&db_path)?
        };

        // Full-text search is built on demand by the search command, not here.
        //
        // Opening the on-disk index eagerly took tantivy's writer lock on every single
        // invocation, including ones that never search (`entity add` created the index
        // directory), which is a second process-wide lock on top of redb's. Remote mode
        // must not take it at all, since the daemon owns the data.
        //
        // Results are unaffected: hits are filtered against the entity slice passed in,
        // and the index rebuilds when the reader sees no documents, so the on-disk copy
        // was providing freshness rather than reuse.

        Ok(Self {
            storage: Arc::new(storage),
            #[cfg(feature = "remote")]
            remote: None,
        })
    }

    /// Resolve a project name to its id, creating the project if it does not exist.
    ///
    /// Remote mode resolves this in one atomic server-side call (`RemoteStorage` overrides
    /// the trait method). Doing it as a client-side get-then-create loses data: two
    /// clients both see no project, both mint a different `ProjectId`, and both save. The
    /// last write wins the name while the loser's entities stay keyed under an id no name
    /// resolves to, so they vanish. That is not theoretical, it is what
    /// `concurrent_clients_agree_on_one_project` reproduces.
    ///
    /// Local backends use the trait's default get-then-create: one process holds the
    /// database, so nothing races with it.
    pub async fn project_id(&self, name: &str) -> anyhow::Result<parsnip_core::ProjectId> {
        Ok(self.storage.get_or_create_project(name).await?.id)
    }
}

/// Turn redb's bare lock message into something that says what to do about it.
///
/// redb allows exactly one process to open a database, so a running `parsnip serve`
/// makes every local invocation fail. The fix is to point this one at that daemon.
#[cfg(feature = "redb")]
fn explain_lock_error(e: parsnip_storage::StorageError, db_path: &Path) -> anyhow::Error {
    let text = e.to_string();
    if !text.contains("already open") {
        return anyhow::anyhow!(e);
    }

    anyhow::anyhow!(
        "The parsnip database is already open by another process (most likely a running \
         `parsnip serve`).\n\
         \n\
         Database: {}\n\
         \n\
         Point this command at that daemon instead of the file:\n  \
           parsnip --server http://127.0.0.1:8787 <command>\n  \
           export PARSNIP_SERVER=http://127.0.0.1:8787\n  \
           parsnip config set server_url http://127.0.0.1:8787\n\
         \n\
         Or stop the other process to use the database directly.",
        db_path.display()
    )
}

/// Whether `command` can render `format`.
///
/// Mirrors what the views support (see `view::render`): table and JSON everywhere, CSV only
/// for row-shaped output, graphml only for export. Checked up front because `emit` runs
/// after the command, and by then a delete or update has already happened.
fn check_format(command: &Commands, format: view::OutputFormat) -> Result<(), String> {
    use commands::entity::EntityCommands;
    use commands::project::ProjectCommands;
    use commands::relation::RelationCommands;
    use view::OutputFormat;

    let supported = match format {
        OutputFormat::Table | OutputFormat::Json => true,
        OutputFormat::Graphml => matches!(
            command,
            Commands::Export(_) | Commands::Import(_) | Commands::Serve(_)
        ),
        OutputFormat::Csv => match command {
            Commands::Entity(args) => matches!(args.command, EntityCommands::List { .. }),
            Commands::Relation(args) => matches!(args.command, RelationCommands::List { .. }),
            Commands::Project(args) => matches!(args.command, ProjectCommands::List),
            Commands::Search(_)
            | Commands::Export(_)
            | Commands::Import(_)
            | Commands::Serve(_) => true,
            Commands::Config(_) | Commands::Completions(_) => true,
        },
    };

    if supported {
        return Ok(());
    }
    Err(match format {
        OutputFormat::Graphml => "graphml output is only supported by `parsnip export`.".into(),
        _ => format!(
            "{} output is not supported for this command; it has no row shape. \
             Use --format json.",
            format.as_str()
        ),
    })
}

/// Resolve `serve`'s host and port to the address to listen on, and refuse to expose the
/// graph beyond loopback without both `--allow-remote` and a token.
///
/// The host is resolved before it is judged: a name such as `localhost` is only trusted if
/// every address it resolves to is loopback, and `127.0.0.2` or `[::1]` count as loopback
/// too. IPv6 literals are accepted with or without brackets.
#[cfg(feature = "sse")]
async fn resolve_bind(
    args: &ServeArgs,
    auth_token: Option<&str>,
) -> anyhow::Result<std::net::SocketAddr> {
    use std::net::{IpAddr, SocketAddr};

    if auth_token.is_some_and(|t| t.trim().is_empty()) {
        anyhow::bail!(
            "The auth token is set but empty (check --auth-token / PARSNIP_AUTH_TOKEN and \
             the file it is read from). Refusing to start."
        );
    }

    let host = args.host.trim_start_matches('[').trim_end_matches(']');
    let addrs: Vec<SocketAddr> = match host.parse::<IpAddr>() {
        Ok(ip) => vec![SocketAddr::new(ip, args.port)],
        Err(_) => tokio::net::lookup_host((host, args.port))
            .await
            .map_err(|e| anyhow::anyhow!("cannot resolve --host {}: {e}", args.host))?
            .collect(),
    };
    // Prefer IPv4 when a name resolves to both families, so `--host localhost` listens where
    // `http://127.0.0.1:<port>` clients will look.
    let Some(first) = addrs
        .iter()
        .find(|a| a.is_ipv4())
        .or_else(|| addrs.first())
        .copied()
    else {
        anyhow::bail!("--host {} resolved to no addresses", args.host);
    };
    let is_loopback = addrs.iter().all(|a| a.ip().is_loopback());
    tracing::debug!(host = %args.host, ?addrs, is_loopback, "resolved serve address");

    if !is_loopback && !args.allow_remote {
        anyhow::bail!(
            "Binding to {} requires --allow-remote flag.\n\
             WARNING: This exposes your knowledge graph to the network!",
            args.host
        );
    }
    if !is_loopback && auth_token.is_none() {
        anyhow::bail!("Non-localhost binding requires --auth-token or PARSNIP_AUTH_TOKEN env var");
    }

    Ok(first)
}

// CLI commands use a current_thread runtime for faster cold start; `serve` needs a
// multi-threaded one because it is a daemon handling concurrent clients whose storage
// calls block on redb I/O.
fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    let runtime = match &cli.command {
        Commands::Serve(_) => tokio::runtime::Runtime::new()?,
        _ => tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?,
    };

    runtime.block_on(run(cli))
}

async fn run(cli: Cli) -> anyhow::Result<()> {
    // Set up logging based on verbosity
    let filter = match cli.verbose {
        0 if cli.quiet => "error",
        0 => "warn",
        1 => "info",
        2 => "debug",
        _ => "trace",
    };

    // Logs go to stderr: stdout carries command output, and for `serve --transport stdio`
    // it carries the JSON-RPC stream, which a stray log line would corrupt.
    tracing_subscriber::registry()
        .with(
            fmt::layer()
                .with_writer(std::io::stderr)
                // No colour codes in log files (e.g. a launchd daemon's StandardErrorPath).
                .with_ansi(std::io::IsTerminal::is_terminal(&std::io::stderr())),
        )
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| filter.into()))
        .init();

    tracing::debug!("Starting parsnip CLI");

    // These commands never touch storage. Handle them before opening the database so they
    // keep working when another process holds the lock.
    match &cli.command {
        Commands::Config(args) => return config_cmd::run(args).await,
        Commands::Completions(args) => return completions::run(args),
        _ => {}
    }

    // Resolve where storage lives: --server, then PARSNIP_SERVER (clap merges those two),
    // then the config file. `--local` overrides all of them for one invocation.
    //
    // Deliberately narrow: only `server_url` is read from the config file. The other keys
    // (default_project, output_format, log_level) are not wired up, because `--project`
    // has a clap default, so an unset flag is indistinguishable from an explicit
    // `-p default`, and silently preferring the config value would change which project
    // every existing command operates on.
    let server_url = if cli.local {
        None
    } else {
        cli.server
            .clone()
            .or_else(|| config::Config::load().server_url)
    };

    // Refuse an output format the command cannot render before anything runs, so a
    // mutation is never performed and then reported as a usage error.
    if let Err(message) = check_format(&cli.command, cli.format) {
        eprintln!("{message}");
        std::process::exit(2);
    }

    // Vet the listen address before opening storage, so a refused bind has no side effects.
    #[cfg(feature = "sse")]
    let sse_bind = match &cli.command {
        Commands::Serve(args) if matches!(args.transport.as_str(), "sse" | "http") => {
            Some(resolve_bind(args, cli.auth_token.as_deref()).await?)
        }
        _ => None,
    };

    // Initialize storage
    let ctx = AppContext::new(&cli, server_url.as_deref()).await?;

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
                    let addr = sse_bind.expect("resolved before storage was opened");
                    tracing::info!("Starting MCP server with SSE transport on {}", addr);
                    parsnip_mcp::run_sse_server(server, &addr.to_string(), cli.auth_token.clone())
                        .await?;
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
        // Handled above, before storage was opened.
        Commands::Config(_) | Commands::Completions(_) => unreachable!(),
    }

    Ok(())
}
