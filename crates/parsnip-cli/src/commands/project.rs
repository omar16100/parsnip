//! Project commands

use clap::{Args, Subcommand};

use crate::{AppContext, Cli};
use parsnip_core::Project;
use parsnip_storage::StorageBackend;

#[derive(Args)]
pub struct ProjectArgs {
    #[command(subcommand)]
    pub command: ProjectCommands,
}

#[derive(Subcommand)]
pub enum ProjectCommands {
    /// List all projects
    List,
    /// Create a new project
    Create {
        /// Project name
        name: String,
        /// Project description
        #[arg(short, long)]
        description: Option<String>,
    },
    /// Set default project
    Use {
        /// Project name
        name: String,
    },
    /// Delete a project
    Delete {
        /// Project name
        name: String,
        /// Force deletion without confirmation
        #[arg(short, long)]
        force: bool,
    },
    /// Show project statistics
    Stats {
        /// Project name (default: current project)
        name: Option<String>,
    },
}

pub async fn run(args: &ProjectArgs, cli: &Cli, ctx: &AppContext) -> anyhow::Result<()> {
    tracing::debug!("Running project command");

    match &args.command {
        ProjectCommands::List => {
            let projects = ctx.storage.get_all_projects().await?;
            tracing::info!("Found {} projects", projects.len());

            if projects.is_empty() {
                println!("No projects found. Create one with 'parsnip project create <name>'");
            } else {
                println!("Projects ({} found):", projects.len());
                for project in &projects {
                    let current = if project.name == cli.project { " (current)" } else { "" };
                    let desc = project.description
                        .as_ref()
                        .map(|d| format!(" - {}", d))
                        .unwrap_or_default();
                    println!("  {}{}{}", project.name, current, desc);
                }
            }
        }
        ProjectCommands::Create { name, description } => {
            // Check if project already exists
            if ctx.storage.get_project(name).await?.is_some() {
                println!("Project '{}' already exists", name);
                return Ok(());
            }

            let mut project = Project::new(name);
            if let Some(desc) = description {
                project = project.with_description(desc);
            }

            ctx.storage.save_project(&project).await?;
            tracing::info!("Created project: {}", name);

            println!("Created project: {}", name);
            if let Some(desc) = description {
                println!("  description: {}", desc);
            }
        }
        ProjectCommands::Use { name } => {
            // Check if project exists
            if ctx.storage.get_project(name).await?.is_none() {
                println!("Project '{}' not found. Create it with 'parsnip project create {}'", name, name);
                return Ok(());
            }

            tracing::info!("Switching to project: {}", name);
            println!("To use project '{}', run commands with: parsnip -p {}", name, name);
            println!("Or set PARSNIP_PROJECT={} in your environment", name);
        }
        ProjectCommands::Delete { name, force } => {
            // Check if project exists
            let project = match ctx.storage.get_project(name).await? {
                Some(p) => p,
                None => {
                    println!("Project '{}' not found", name);
                    return Ok(());
                }
            };

            // Get entity count for warning
            let entity_count = ctx.storage.get_all_entities(&project.id).await?.len();
            let relation_count = ctx.storage.get_all_relations(&project.id).await?.len();

            if !force {
                println!("Project '{}' has {} entities and {} relations", name, entity_count, relation_count);
                println!("Use --force to confirm deletion");
                return Ok(());
            }

            ctx.storage.delete_project(name).await?;
            tracing::info!("Deleted project: {} ({} entities, {} relations)", name, entity_count, relation_count);
            println!("Deleted project: {} ({} entities, {} relations)", name, entity_count, relation_count);
        }
        ProjectCommands::Stats { name } => {
            let project_name = name.as_deref().unwrap_or(&cli.project);

            let project = match ctx.storage.get_project(project_name).await? {
                Some(p) => p,
                None => {
                    println!("Project '{}' not found", project_name);
                    return Ok(());
                }
            };

            let entities = ctx.storage.get_all_entities(&project.id).await?;
            let relations = ctx.storage.get_all_relations(&project.id).await?;

            // Count entities by type
            let mut type_counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
            let mut total_observations = 0;
            let mut total_tags = 0;

            for entity in &entities {
                *type_counts.entry(entity.entity_type.0.clone()).or_insert(0) += 1;
                total_observations += entity.observations.len();
                total_tags += entity.tags.len();
            }

            // Count relations by type
            let mut rel_type_counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
            for relation in &relations {
                *rel_type_counts.entry(relation.relation_type.clone()).or_insert(0) += 1;
            }

            tracing::info!("Stats for project: {}", project_name);

            println!("Stats for project '{}':", project_name);
            if let Some(desc) = &project.description {
                println!("  Description: {}", desc);
            }
            println!("  Created: {}", project.created_at);
            println!();
            println!("  Entities: {}", entities.len());
            for (entity_type, count) in type_counts.iter() {
                println!("    {}: {}", entity_type, count);
            }
            println!();
            println!("  Observations: {}", total_observations);
            println!("  Tags: {}", total_tags);
            println!();
            println!("  Relations: {}", relations.len());
            for (rel_type, count) in rel_type_counts.iter() {
                println!("    {}: {}", rel_type, count);
            }
        }
    }

    Ok(())
}
