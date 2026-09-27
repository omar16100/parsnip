//! Project commands

use clap::{Args, Subcommand};

use crate::view::{emit, CountRow, MutationView, ProjectListView, ProjectRow, ProjectStatsView};
use crate::{AppContext, Cli};

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
        // No short form: -d is taken by the global --data-dir.
        #[arg(long)]
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
        // No short form: -f is taken by the global --format.
        #[arg(long)]
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

            let view = ProjectListView {
                projects: projects
                    .iter()
                    .map(|p| ProjectRow {
                        name: p.name.clone(),
                        description: p.description.clone(),
                        current: p.name == cli.project,
                    })
                    .collect(),
            };
            emit(&view, cli.format)?;
        }
        ProjectCommands::Create { name, description } => {
            // Check if project already exists
            if ctx.storage.get_project(name).await?.is_some() {
                println!("Project '{}' already exists", name);
                return Ok(());
            }

            // Created through the atomic path rather than minting an id here: if another
            // client created the same name in the meantime, reuse its id instead of
            // rebinding the name and orphaning that client's entities.
            let mut project = ctx.storage.get_or_create_project(name).await?;
            if let Some(desc) = description {
                if project.description.is_none() {
                    project = project.with_description(desc);
                    ctx.storage.save_project(&project).await?;
                }
            }
            tracing::info!("Created project: {}", name);

            let details = description
                .as_ref()
                .map(|d| vec![format!("  description: {}", d)])
                .unwrap_or_default();
            emit(
                &MutationView::with_details(format!("Created project: {}", name), details),
                cli.format,
            )?;
        }
        ProjectCommands::Use { name } => {
            // Check if project exists
            if ctx.storage.get_project(name).await?.is_none() {
                println!(
                    "Project '{}' not found. Create it with 'parsnip project create {}'",
                    name, name
                );
                return Ok(());
            }

            // Save to config file
            let mut config = crate::config::Config::load();
            config.default_project = name.clone();
            config.save()?;

            tracing::info!("Set default project to: {}", name);
            println!("Default project set to: {}", name);
            println!(
                "Config saved to: {}",
                crate::config::config_file_path().display()
            );
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
                println!(
                    "Project '{}' has {} entities and {} relations",
                    name, entity_count, relation_count
                );
                println!("Use --force to confirm deletion");
                return Ok(());
            }

            ctx.storage.delete_project(name).await?;
            tracing::info!(
                "Deleted project: {} ({} entities, {} relations)",
                name,
                entity_count,
                relation_count
            );
            emit(
                &MutationView::new(format!(
                    "Deleted project: {} ({} entities, {} relations)",
                    name, entity_count, relation_count
                )),
                cli.format,
            )?;
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
            let mut type_counts: std::collections::HashMap<String, usize> =
                std::collections::HashMap::new();
            let mut total_observations = 0;
            let mut total_tags = 0;

            for entity in &entities {
                *type_counts.entry(entity.entity_type.0.clone()).or_insert(0) += 1;
                total_observations += entity.observations.len();
                total_tags += entity.tags.len();
            }

            // Count relations by type
            let mut rel_type_counts: std::collections::HashMap<String, usize> =
                std::collections::HashMap::new();
            for relation in &relations {
                *rel_type_counts
                    .entry(relation.relation_type.clone())
                    .or_insert(0) += 1;
            }

            tracing::info!("Stats for project: {}", project_name);

            // Sorted: these came out of HashMaps, so two runs over identical data
            // printed the breakdown lines in different orders.
            let to_sorted_rows = |counts: std::collections::HashMap<String, usize>| {
                let mut rows: Vec<CountRow> = counts
                    .into_iter()
                    .map(|(name, count)| CountRow { name, count })
                    .collect();
                rows.sort_by(|a, b| a.name.cmp(&b.name));
                rows
            };

            let view = ProjectStatsView {
                project: project_name.to_string(),
                description: project.description.clone(),
                created_at: project.created_at.to_string(),
                entity_count: entities.len(),
                entities_by_type: to_sorted_rows(type_counts),
                observation_count: total_observations,
                tag_count: total_tags,
                relation_count: relations.len(),
                relations_by_type: to_sorted_rows(rel_type_counts),
            };
            emit(&view, cli.format)?;
        }
    }

    Ok(())
}
