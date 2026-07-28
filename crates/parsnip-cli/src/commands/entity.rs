//! Entity commands

use clap::{Args, Subcommand};

use crate::view::{
    emit, EntityDetailView, EntityListView, EntityRow, MutationView, ObservationRow,
};
use crate::{AppContext, Cli};
use parsnip_core::{Entity, ProjectId};

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
        // No short form: -f is taken by the global --format.
        #[arg(long)]
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

/// Delegates to [`AppContext::project_id`], which is atomic in remote mode.
async fn get_project_id(project_name: &str, ctx: &AppContext) -> anyhow::Result<ProjectId> {
    ctx.project_id(project_name).await
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

            let mut details: Vec<String> = obs.iter().map(|o| format!("  - {}", o)).collect();
            details.extend(tag.iter().map(|t| format!("  tag: {}", t)));
            emit(
                &MutationView::with_details(
                    format!("Created entity: {} (type: {})", name, r#type),
                    details,
                ),
                cli.format,
            )?;
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

            let view = EntityListView {
                project: cli.project.clone(),
                entities: filtered
                    .iter()
                    .map(|e| EntityRow {
                        name: e.name.clone(),
                        entity_type: e.entity_type.0.clone(),
                        tags: e.tags.clone(),
                    })
                    .collect(),
            };
            emit(&view, cli.format)?;
        }
        EntityCommands::Get { name } => {
            let project_id = get_project_id(&cli.project, ctx).await?;

            match ctx.storage.get_entity(name, &project_id).await? {
                Some(entity) => {
                    tracing::info!("Found entity: {}", name);
                    let view = EntityDetailView {
                        name: entity.name.clone(),
                        entity_type: entity.entity_type.0.clone(),
                        project: cli.project.clone(),
                        created_at: entity.created_at.to_string(),
                        updated_at: entity.updated_at.to_string(),
                        tags: entity.tags.clone(),
                        observations: entity
                            .observations
                            .iter()
                            .map(|o| ObservationRow {
                                content: o.content.clone(),
                                created_at: o.created_at.to_string(),
                            })
                            .collect(),
                    };
                    emit(&view, cli.format)?;
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
            emit(
                &MutationView::new(format!("Deleted entity: {}", name)),
                cli.format,
            )?;
        }
        EntityCommands::Observe { name, content } => {
            let project_id = get_project_id(&cli.project, ctx).await?;

            match ctx.storage.get_entity(name, &project_id).await? {
                Some(mut entity) => {
                    entity.add_observation(content);
                    ctx.storage.save_entity(&entity).await?;
                    tracing::info!("Added observation to entity: {}", name);
                    emit(
                        &MutationView::new(format!("Added observation to {}: {}", name, content)),
                        cli.format,
                    )?;
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

                    emit(
                        &MutationView::with_details(
                            format!("Updated entity '{}':", name),
                            changes.iter().map(|c| format!("  - {}", c)).collect(),
                        ),
                        cli.format,
                    )?;
                }
                None => {
                    println!("Entity '{}' not found in project '{}'", name, cli.project);
                }
            }
        }
    }

    Ok(())
}
