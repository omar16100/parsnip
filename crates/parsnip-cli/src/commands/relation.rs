//! Relation commands

use std::collections::HashMap;

use clap::{Args, Subcommand};

use crate::{AppContext, Cli};
use parsnip_core::{Direction, ProjectId, Relation, TraversalEngine, TraversalQuery};
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
        /// Filter by relation types (comma-separated)
        #[arg(short = 'r', long)]
        relation_types: Option<String>,
        /// Filter by entity types (comma-separated)
        #[arg(short = 'e', long)]
        entity_types: Option<String>,
    },
    /// Find path between two entities
    FindPath {
        /// Starting entity
        from: String,
        /// Target entity
        to: String,
        /// Use weighted shortest path (Dijkstra)
        #[arg(long)]
        weighted: bool,
        /// Filter by relation types (comma-separated)
        #[arg(short = 'r', long)]
        relation_types: Option<String>,
        /// Filter by entity types (comma-separated)
        #[arg(short = 'e', long)]
        entity_types: Option<String>,
        /// Maximum search depth
        #[arg(long, default_value = "10")]
        max_depth: u32,
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
        RelationCommands::Traverse { start, depth, direction, relation_types, entity_types } => {
            let project_id = get_project_id(&cli.project, ctx).await?;

            // Check if starting entity exists
            if ctx.storage.get_entity(start, &project_id).await?.is_none() {
                println!("Entity '{}' not found in project '{}'", start, cli.project);
                return Ok(());
            }

            // Parse direction
            let dir = match direction.as_str() {
                "outgoing" => Direction::Outgoing,
                "incoming" => Direction::Incoming,
                _ => Direction::Both,
            };

            // Build query
            let mut query = TraversalQuery::new(start).with_depth(*depth).with_direction(dir);

            if let Some(ref rtypes) = relation_types {
                let types: Vec<String> = rtypes.split(',').map(|s| s.trim().to_string()).collect();
                query = query.filter_relation_types(types);
            }

            if let Some(ref etypes) = entity_types {
                let types: Vec<String> = etypes.split(',').map(|s| s.trim().to_string()).collect();
                query = query.filter_entity_types(types);
            }

            // Load data
            let entities: HashMap<String, _> = ctx.storage.get_all_entities(&project_id).await?
                .into_iter().map(|e| (e.name.clone(), e)).collect();
            let relations = ctx.storage.get_all_relations(&project_id).await?;

            tracing::info!("Traversing from {} (depth: {}, direction: {})", start, depth, direction);

            // Execute traversal
            let result = TraversalEngine::execute(&query, &entities, &relations);

            println!("Traversal from '{}' (depth: {}, direction: {}):", start, depth, direction);
            println!("  Visited {} entities, traversed {} edges",
                result.stats.nodes_visited, result.stats.edges_traversed);

            if result.visited_entities.len() <= 1 {
                println!("  (no connected entities found)");
            } else {
                println!("  Entities: {}", result.visited_entities.join(", "));

                if !result.relations.is_empty() {
                    println!("  Relations:");
                    for rel in &result.relations {
                        let weight_str = rel.weight
                            .map(|w| format!(" (weight: {:.2})", w))
                            .unwrap_or_default();
                        println!("    {} -[{}]-> {}{}",
                            rel.from_name, rel.relation_type, rel.to_name, weight_str);
                    }
                }
            }
        }
        RelationCommands::FindPath { from, to, weighted, relation_types, entity_types, max_depth } => {
            let project_id = get_project_id(&cli.project, ctx).await?;

            // Check if both entities exist
            if ctx.storage.get_entity(from, &project_id).await?.is_none() {
                println!("Entity '{}' not found in project '{}'", from, cli.project);
                return Ok(());
            }
            if ctx.storage.get_entity(to, &project_id).await?.is_none() {
                println!("Entity '{}' not found in project '{}'", to, cli.project);
                return Ok(());
            }

            // Build query
            let mut query = TraversalQuery::new(from)
                .find_path_to(to)
                .with_depth(*max_depth);

            if *weighted {
                query = query.weighted();
            }

            if let Some(ref rtypes) = relation_types {
                let types: Vec<String> = rtypes.split(',').map(|s| s.trim().to_string()).collect();
                query = query.filter_relation_types(types);
            }

            if let Some(ref etypes) = entity_types {
                let types: Vec<String> = etypes.split(',').map(|s| s.trim().to_string()).collect();
                query = query.filter_entity_types(types);
            }

            // Load data
            let entities: HashMap<String, _> = ctx.storage.get_all_entities(&project_id).await?
                .into_iter().map(|e| (e.name.clone(), e)).collect();
            let relations = ctx.storage.get_all_relations(&project_id).await?;

            tracing::info!("Finding path from {} to {} (weighted: {}, max_depth: {})",
                from, to, weighted, max_depth);

            // Execute path finding
            let result = TraversalEngine::execute(&query, &entities, &relations);

            if result.paths.is_empty() {
                println!("No path found from '{}' to '{}'", from, to);
                println!("  (searched {} nodes, {} edges)",
                    result.stats.nodes_visited, result.stats.edges_traversed);
            } else {
                let algo = if *weighted { "Dijkstra" } else { "BFS" };
                println!("Path found from '{}' to '{}' using {}:", from, to, algo);

                for (i, path) in result.paths.iter().enumerate() {
                    println!("\n  Path {}: {} hops, total weight: {:.2}",
                        i + 1, path.length, path.total_weight);
                    println!("  Route: {}", path.nodes.join(" -> "));

                    for edge in &path.edges {
                        let weight_str = edge.weight
                            .map(|w| format!(" (weight: {:.2})", w))
                            .unwrap_or_default();
                        println!("    {} -[{}]-> {}{}",
                            edge.from, edge.relation_type, edge.to, weight_str);
                    }
                }

                println!("\n  Stats: visited {} nodes, traversed {} edges",
                    result.stats.nodes_visited, result.stats.edges_traversed);
            }
        }
    }

    Ok(())
}
