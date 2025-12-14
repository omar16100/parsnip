//! Relation commands

use clap::{Args, Subcommand};

use crate::{AppContext, Cli};
use parsnip_core::{ProjectId, Relation};
use parsnip_storage::StorageBackend;

#[derive(Args)]
pub struct RelationArgs {
    #[command(subcommand)]
    pub command: RelationCommands,
}

#[derive(Subcommand)]
pub enum RelationCommands {
    /// Add a new relation
    Add {
        /// Source entity
        from: String,
        /// Target entity
        to: String,
        /// Relation type
        #[arg(short = 't', long)]
        r#type: String,
        /// Relation weight
        #[arg(short, long)]
        weight: Option<f64>,
    },
    /// List relations
    List {
        /// Filter by source entity
        #[arg(long)]
        from: Option<String>,
        /// Filter by target entity
        #[arg(long)]
        to: Option<String>,
        /// Filter by type
        #[arg(short = 't', long)]
        r#type: Option<String>,
    },
    /// Delete a relation
    Delete {
        /// Source entity
        from: String,
        /// Target entity
        to: String,
        /// Relation type
        #[arg(short = 't', long)]
        r#type: String,
    },
    /// Traverse graph from an entity
    Traverse {
        /// Starting entity
        start: String,
        /// Traversal depth
        #[arg(short, long, default_value = "2")]
        depth: u32,
        /// Direction: outgoing, incoming, both
        #[arg(long, default_value = "both")]
        direction: String,
    },
}

async fn get_project_id(project_name: &str, ctx: &AppContext) -> anyhow::Result<ProjectId> {
    if let Some(project) = ctx.storage.get_project(project_name).await? {
        return Ok(project.id);
    }
    let project = parsnip_core::Project::new(project_name);
    ctx.storage.save_project(&project).await?;
    tracing::info!("Created new project: {}", project_name);
    Ok(project.id)
}

pub async fn run(args: &RelationArgs, cli: &Cli, ctx: &AppContext) -> anyhow::Result<()> {
    tracing::debug!("Running relation command for project: {}", cli.project);

    match &args.command {
        RelationCommands::Add { from, to, r#type, weight } => {
            let project_id = get_project_id(&cli.project, ctx).await?;

            // Check if both entities exist
            if ctx.storage.get_entity(from, &project_id).await?.is_none() {
                println!("Source entity '{}' not found in project '{}'", from, cli.project);
                return Ok(());
            }
            if ctx.storage.get_entity(to, &project_id).await?.is_none() {
                println!("Target entity '{}' not found in project '{}'", to, cli.project);
                return Ok(());
            }

            let mut relation = Relation::from_names(project_id, from, to, r#type);
            if let Some(w) = weight {
                relation = relation.with_weight(*w);
            }

            ctx.storage.save_relation(&relation).await?;
            tracing::info!("Created relation: {} -[{}]-> {}", from, r#type, to);

            println!("Created relation: {} -[{}]-> {}", from, r#type, to);
            if let Some(w) = weight {
                println!("  weight: {}", w);
            }
        }
        RelationCommands::List { from, to, r#type } => {
            let project_id = get_project_id(&cli.project, ctx).await?;
            let relations = ctx.storage.get_all_relations(&project_id).await?;

            let filtered: Vec<_> = relations
                .into_iter()
                .filter(|r| {
                    if let Some(f) = from {
                        if r.from_name != *f {
                            return false;
                        }
                    }
                    if let Some(t) = to {
                        if r.to_name != *t {
                            return false;
                        }
                    }
                    if let Some(rt) = r#type {
                        if r.relation_type.to_lowercase() != rt.to_lowercase() {
                            return false;
                        }
                    }
                    true
                })
                .collect();

            tracing::info!("Found {} relations", filtered.len());

            if filtered.is_empty() {
                println!("No relations found in project '{}'", cli.project);
            } else {
                println!("Relations in project '{}' ({} found):", cli.project, filtered.len());
                for relation in &filtered {
                    let weight_str = relation.weight
                        .map(|w| format!(" (weight: {:.2})", w))
                        .unwrap_or_default();
                    println!("  {} -[{}]-> {}{}",
                        relation.from_name,
                        relation.relation_type,
                        relation.to_name,
                        weight_str
                    );
                }
            }
        }
        RelationCommands::Delete { from, to, r#type } => {
            let project_id = get_project_id(&cli.project, ctx).await?;

            ctx.storage.delete_relation(from, to, r#type, &project_id).await?;
            tracing::info!("Deleted relation: {} -[{}]-> {}", from, r#type, to);
            println!("Deleted relation: {} -[{}]-> {}", from, r#type, to);
        }
        RelationCommands::Traverse { start, depth, direction } => {
            let project_id = get_project_id(&cli.project, ctx).await?;

            // Check if starting entity exists
            if ctx.storage.get_entity(start, &project_id).await?.is_none() {
                println!("Entity '{}' not found in project '{}'", start, cli.project);
                return Ok(());
            }

            let relations = ctx.storage.get_all_relations(&project_id).await?;
            tracing::info!("Traversing from {} (depth: {}, direction: {})", start, depth, direction);

            println!("Traversal from '{}' (depth: {}, direction: {}):", start, depth, direction);

            // Simple BFS traversal
            let mut visited: std::collections::HashSet<String> = std::collections::HashSet::new();
            let mut queue: Vec<(String, u32)> = vec![(start.clone(), 0)];

            while let Some((current, current_depth)) = queue.pop() {
                if current_depth >= *depth || visited.contains(&current) {
                    continue;
                }
                visited.insert(current.clone());

                for rel in &relations {
                    let next = match direction.as_str() {
                        "outgoing" if rel.from_name == current => Some(&rel.to_name),
                        "incoming" if rel.to_name == current => Some(&rel.from_name),
                        "both" => {
                            if rel.from_name == current {
                                Some(&rel.to_name)
                            } else if rel.to_name == current {
                                Some(&rel.from_name)
                            } else {
                                None
                            }
                        }
                        _ => None,
                    };

                    if let Some(next_entity) = next {
                        if !visited.contains(next_entity) {
                            let indent = "  ".repeat((current_depth + 1) as usize);
                            let arrow = if rel.from_name == current {
                                format!("{} -[{}]-> {}", current, rel.relation_type, next_entity)
                            } else {
                                format!("{} <-[{}]- {}", current, rel.relation_type, next_entity)
                            };
                            println!("{}{}", indent, arrow);
                            queue.push((next_entity.clone(), current_depth + 1));
                        }
                    }
                }
            }

            if visited.len() <= 1 {
                println!("  (no connected entities found)");
            }
        }
    }

    Ok(())
}
