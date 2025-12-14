//! Search commands

use clap::Args;

use crate::{AppContext, Cli};
use parsnip_core::{ProjectId, SearchMode, SearchQuery};
use parsnip_search::{ExactSearchEngine, FuzzySearchEngine, SearchEngine};
use parsnip_storage::StorageBackend;

#[derive(Args)]
pub struct SearchArgs {
    /// Search query
    pub query: Option<String>,

    /// Search all projects
    #[arg(long)]
    pub all_projects: bool,

    /// Enable fuzzy search
    #[arg(long)]
    pub fuzzy: bool,

    /// Fuzzy threshold (0.0-1.0)
    #[arg(long, default_value = "0.3")]
    pub threshold: f32,

    /// Filter by entity type
    #[arg(short = 't', long)]
    pub r#type: Option<String>,

    /// Filter by tag (can be used multiple times)
    #[arg(long)]
    pub tag: Vec<String>,

    /// Search mode: exact, fuzzy, fulltext, hybrid
    #[arg(long, default_value = "exact")]
    pub mode: String,

    /// Limit results
    #[arg(short, long, default_value = "100")]
    pub limit: usize,

    /// Include relations in output
    #[arg(long)]
    pub include_relations: bool,
}

async fn get_project_id(project_name: &str, ctx: &AppContext) -> anyhow::Result<ProjectId> {
    if let Some(project) = ctx.storage.get_project(project_name).await? {
        return Ok(project.id);
    }
    let project = parsnip_core::Project::new(project_name);
    ctx.storage.save_project(&project).await?;
    Ok(project.id)
}

pub async fn run(args: &SearchArgs, cli: &Cli, ctx: &AppContext) -> anyhow::Result<()> {
    let scope = if args.all_projects { "all projects" } else { &cli.project };

    // Build search query
    let mut query = if let Some(ref q) = args.query {
        SearchQuery::new(q)
    } else if !args.tag.is_empty() {
        SearchQuery::empty()
    } else {
        println!("Please provide a search query or tags");
        return Ok(());
    };

    // Apply filters
    if let Some(ref t) = args.r#type {
        query = query.with_entity_type(t);
    }
    for tag in &args.tag {
        query = query.with_tag(tag);
    }

    // Set search mode
    let mode = match args.mode.as_str() {
        "fuzzy" => SearchMode::Fuzzy,
        "fulltext" => SearchMode::FullText,
        "hybrid" => SearchMode::Hybrid,
        _ => SearchMode::Exact,
    };
    query = query.with_mode(mode);

    if args.fuzzy {
        query = query.with_mode(SearchMode::Fuzzy).with_fuzzy_threshold(args.threshold);
    }

    // Set project scope
    if args.all_projects {
        query = query.in_all_projects();
    } else {
        let project_id = get_project_id(&cli.project, ctx).await?;
        query = query.in_project(project_id);
    }

    // Get entities to search
    let entities = if args.all_projects {
        ctx.storage.get_all_entities_all_projects().await?
    } else {
        let project_id = get_project_id(&cli.project, ctx).await?;
        ctx.storage.get_all_entities(&project_id).await?
    };

    // Perform search based on mode
    let results = match query.mode {
        SearchMode::Fuzzy => {
            let search_engine = FuzzySearchEngine::new();
            search_engine.search(&query, &entities).await?
        }
        #[cfg(feature = "fulltext")]
        SearchMode::FullText | SearchMode::Hybrid => {
            if let Some(ref fulltext) = ctx.fulltext {
                use parsnip_search::SearchEngine;
                fulltext.search(&query, &entities).await?
            } else {
                tracing::warn!("Full-text search not available, falling back to exact search");
                let search_engine = ExactSearchEngine::new();
                search_engine.search(&query, &entities).await?
            }
        }
        #[cfg(not(feature = "fulltext"))]
        SearchMode::FullText | SearchMode::Hybrid => {
            tracing::warn!("Full-text search not enabled, falling back to exact search");
            let search_engine = ExactSearchEngine::new();
            search_engine.search(&query, &entities).await?
        }
        _ => {
            let search_engine = ExactSearchEngine::new();
            search_engine.search(&query, &entities).await?
        }
    };

    let display_results: Vec<_> = results.into_iter().take(args.limit).collect();

    tracing::info!("Search returned {} results in {}", display_results.len(), scope);

    if display_results.is_empty() {
        println!("No results found in {}", scope);
    } else {
        if let Some(ref q) = args.query {
            println!("Search results for '{}' in {} ({} found):", q, scope, display_results.len());
        } else {
            println!("Search results for tags {:?} in {} ({} found):", args.tag, scope, display_results.len());
        }

        for entity in &display_results {
            let tags = if entity.tags.is_empty() {
                String::new()
            } else {
                format!(" [{}]", entity.tags.join(", "))
            };
            println!("  {} ({}){}",
                entity.name,
                entity.entity_type.0,
                tags
            );

            if args.include_relations {
                let project_id = &entity.project_id;
                if let Ok(relations) = ctx.storage.get_all_relations(project_id).await {
                    let related: Vec<_> = relations
                        .iter()
                        .filter(|r| r.from_name == entity.name || r.to_name == entity.name)
                        .collect();
                    if !related.is_empty() {
                        for rel in related {
                            if rel.from_name == entity.name {
                                println!("    -> {} ({})", rel.to_name, rel.relation_type);
                            } else {
                                println!("    <- {} ({})", rel.from_name, rel.relation_type);
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(())
}
