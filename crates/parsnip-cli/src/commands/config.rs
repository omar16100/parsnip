//! Config command for managing CLI configuration

use clap::{Args, Subcommand};

use crate::config::{config_file_path, Config};

#[derive(Args)]
pub struct ConfigArgs {
    #[command(subcommand)]
    pub command: ConfigCommands,
}

#[derive(Subcommand)]
pub enum ConfigCommands {
    /// Get a config value
    Get {
        /// Config key name
        key: String,
    },
    /// Set a config value
    Set {
        /// Config key name
        key: String,
        /// New value
        value: String,
    },
    /// List all config values
    List,
    /// Show config file path
    Path,
    /// Initialize default config file
    Init {
        /// Overwrite existing config
        #[arg(long)]
        force: bool,
    },
}

pub async fn run(args: &ConfigArgs) -> anyhow::Result<()> {
    match &args.command {
        ConfigCommands::Get { key } => run_get(key),
        ConfigCommands::Set { key, value } => run_set(key, value),
        ConfigCommands::List => run_list(),
        ConfigCommands::Path => run_path(),
        ConfigCommands::Init { force } => run_init(*force),
    }
}

fn run_get(key: &str) -> anyhow::Result<()> {
    let config = Config::load();
    match config.get(key) {
        Some(value) => println!("{}", value),
        None => {
            eprintln!("Unknown config key: {}", key);
            eprintln!("Available keys: {}", Config::keys().join(", "));
            std::process::exit(1);
        }
    }
    Ok(())
}

fn run_set(key: &str, value: &str) -> anyhow::Result<()> {
    let mut config = Config::load();
    config.set(key, value)?;
    config.save()?;
    println!("Set {} = {}", key, value);
    Ok(())
}

fn run_list() -> anyhow::Result<()> {
    let config = Config::load();
    println!("Config file: {}", config_file_path().display());
    println!();
    for key in Config::keys() {
        let value = config.get(key).unwrap_or_else(|| "(not set)".to_string());
        println!("{} = {}", key, value);
    }
    Ok(())
}

fn run_path() -> anyhow::Result<()> {
    println!("{}", config_file_path().display());
    Ok(())
}

fn run_init(force: bool) -> anyhow::Result<()> {
    let path = config_file_path();

    if path.exists() && !force {
        anyhow::bail!(
            "Config file already exists at {}. Use --force to overwrite.",
            path.display()
        );
    }

    let config = Config::default();
    config.save()?;
    println!("Created config file at {}", path.display());
    Ok(())
}
