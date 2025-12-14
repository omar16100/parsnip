//! Entity commands

use clap::{Args, Subcommand};

use crate::{AppContext, Cli};
use parsnip_core::{Entity, ProjectId};
use parsnip_storage::StorageBackend;

#[derive(Args)]
pub struct EntityArgs {
    #[command(subcommand)]
    pub command: EntityCommands,
}

#[derive(Subcommand)]
pub enum EntityCommands {
    /// Add a new entity
    Add {
        /// Entity name
        name: String,
        /// Entity type
        #[arg(short = 't', long)]
        r#type: String,
        /// Observations about the entity
        #[arg(short, long)]
        obs: Vec<String>,
        /// Tags for the entity
        #[arg(long)]
        tag: Vec<String>,
    },
    /// List entities
    List {
        /// Filter by type
        #[arg(short = 't', long)]
        r#type: Option<String>,
        /// Filter by tag
        #[arg(long)]
        tag: Option<String>,
        /// Limit results
        #[arg(short, long, default_value = "100")]
        limit: usize,
    },
    /// Get entity details
    Get {
        /// Entity name
        name: String,
    },
    /// Delete an entity
    Delete {
        /// Entity name
        name: String,
        /// Force deletion without confirmation
        #[arg(short, long)]
        force: bool,
    },
    /// Add observation to entity
    Observe {
        /// Entity name
        name: String,
        /// Observation content
        content: String,
    },
    /// Update an existing entity
    Update {
        /// Entity name
        name: String,
        /// Add observations
        #[arg(long = "add-obs")]
        add_obs: Vec<String>,
        /// Add tags
        #[arg(long = "add-tag")]
        add_tag: Vec<String>,
        /// Remove tags
        #[arg(long = "remove-tag")]
        remove_tag: Vec<String>,
        /// Set entity type
        #[arg(long = "set-type")]
        set_type: Option<String>,
    },
}

async fn get_project_id(project_name: &str, ctx: &AppContext) -> anyhow::Result<ProjectId> {
    // Try to find existing project
    if let Some(project) = ctx.storage.get_project(project_name).await? {
        return Ok(project.id);
    }

    // Create new project if it doesn't exist
    let project = parsnip_core::Project::new(project_name);
    ctx.storage.save_project(&project).await?;
    tracing::info!("Created new project: {}", project_name);
    Ok(project.id)
}

pub async fn run(args: &EntityArgs, cli: &Cli, ctx: &AppContext) -> anyhow::Result<()> {
    tracing::debug!("Running entity command for project: {}", cli.project);

    match &args.command {
        EntityCommands::Add {
            name,
            r#type,
            obs,
            tag,
        } => {
            let project_id = get_project_id(&cli.project, ctx).await?;

            let mut entity = Entity::new(project_id, name, r#type.as_str());
            for observation in obs {
                entity.add_observation(observation);
            }
            for t in tag {
                entity.add_tag(t);
            }

            ctx.storage.save_entity(&entity).await?;
            tracing::info!("Created entity: {} (type: {})", name, r#type);

            println!("Created entity: {} (type: {})", name, r#type);
            for o in obs {
                println!("  - {}", o);
            }
            for t in tag {
                println!("  tag: {}", t);
            }
        }
        EntityCommands::List { r#type, tag, limit } => {
            let project_id = get_project_id(&cli.project, ctx).await?;
            let entities = ctx.storage.get_all_entities(&project_id).await?;

            let filtered: Vec<_> = entities
                .into_iter()
                .filter(|e| {
                    if let Some(t) = r#type {
                        if e.entity_type.0.to_lowercase() != t.to_lowercase() {
                            return false;
                        }
                    }
                    if let Some(t) = tag {
                        if !e
                            .tags
                            .iter()
                            .any(|et| et.to_lowercase() == t.to_lowercase())
                        {
                            return false;
                        }
                    }
                    true
                })
                .take(*limit)
                .collect();

            tracing::info!("Found {} entities", filtered.len());

            if filtered.is_empty() {
                println!("No entities found in project '{}'", cli.project);
            } else {
                println!(
                    "Entities in project '{}' ({} found):",
                    cli.project,
                    filtered.len()
                );
                for entity in &filtered {
                    let tags = if entity.tags.is_empty() {
                        String::new()
                    } else {
                        format!(" [{}]", entity.tags.join(", "))
                    };
                    println!("  {} ({}){}", entity.name, entity.entity_type.0, tags);
                }
            }
        }
        EntityCommands::Get { name } => {
            let project_id = get_project_id(&cli.project, ctx).await?;

            match ctx.storage.get_entity(name, &project_id).await? {
                Some(entity) => {
                    tracing::info!("Found entity: {}", name);
                    println!("Entity: {}", entity.name);
                    println!("  Type: {}", entity.entity_type.0);
                    println!("  Project: {}", cli.project);
                    println!("  Created: {}", entity.created_at);
                    println!("  Updated: {}", entity.updated_at);

                    if !entity.tags.is_empty() {
                        println!("  Tags: {}", entity.tags.join(", "));
                    }

                    if !entity.observations.is_empty() {
                        println!("  Observations:");
                        for obs in &entity.observations {
                            println!("    - {} ({})", obs.content, obs.created_at);
                        }
                    }
                }
                None => {
                    println!("Entity '{}' not found in project '{}'", name, cli.project);
                }
            }
        }
        EntityCommands::Delete { name, force } => {
            let project_id = get_project_id(&cli.project, ctx).await?;

            if !force {
                // Check if entity exists first
                if ctx.storage.get_entity(name, &project_id).await?.is_none() {
                    println!("Entity '{}' not found in project '{}'", name, cli.project);
                    return Ok(());
                }

                println!("Use --force to confirm deletion of entity '{}'", name);
                return Ok(());
            }

            ctx.storage.delete_entity(name, &project_id).await?;
            tracing::info!("Deleted entity: {}", name);
            println!("Deleted entity: {}", name);
        }
        EntityCommands::Observe { name, content } => {
            let project_id = get_project_id(&cli.project, ctx).await?;

            match ctx.storage.get_entity(name, &project_id).await? {
                Some(mut entity) => {
                    entity.add_observation(content);
                    ctx.storage.save_entity(&entity).await?;
                    tracing::info!("Added observation to entity: {}", name);
                    println!("Added observation to {}: {}", name, content);
                }
                None => {
                    println!("Entity '{}' not found in project '{}'", name, cli.project);
                }
            }
        }
        EntityCommands::Update {
            name,
            add_obs,
            add_tag,
            remove_tag,
            set_type,
        } => {
            let project_id = get_project_id(&cli.project, ctx).await?;

            match ctx.storage.get_entity(name, &project_id).await? {
                Some(mut entity) => {
                    let mut changes = Vec::new();

                    // Add observations
                    for obs in add_obs {
                        entity.add_observation(obs);
                        changes.push(format!("added observation: {}", obs));
                    }

                    // Add tags
                    for tag in add_tag {
                        entity.add_tag(tag);
                        changes.push(format!("added tag: {}", tag));
                    }

                    // Remove tags
                    for tag in remove_tag {
                        if entity.remove_tag(tag) {
                            changes.push(format!("removed tag: {}", tag));
                        } else {
                            println!("Tag '{}' not found on entity", tag);
                        }
                    }

                    // Set type
                    if let Some(new_type) = set_type {
                        entity.entity_type = parsnip_core::EntityType::new(new_type);
                        changes.push(format!("set type: {}", new_type));
                    }

                    if changes.is_empty() {
                        println!("No changes specified");
                        return Ok(());
                    }

                    ctx.storage.save_entity(&entity).await?;
                    tracing::info!("Updated entity '{}': {:?}", name, changes);

                    println!("Updated entity '{}':", name);
                    for change in &changes {
                        println!("  - {}", change);
                    }
                }
                None => {
                    println!("Entity '{}' not found in project '{}'", name, cli.project);
                }
            }
        }
    }

    Ok(())
}
