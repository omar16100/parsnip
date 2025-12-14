//! Import/Export commands

use std::io::Write;
use std::path::PathBuf;

#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;

use clap::Args;
use serde::{Deserialize, Serialize};

use crate::{AppContext, Cli};
use parsnip_core::{Entity, Project, Relation};
use parsnip_storage::StorageBackend;

#[derive(Args)]
pub struct ImportArgs {
    /// Input file (JSON format)
    pub file: PathBuf,

    /// Target project (default: use project from file or 'default')
    #[arg(short = 'p', long)]
    pub target_project: Option<String>,

    /// Merge with existing data (default: error if exists)
    #[arg(long)]
    pub merge: bool,
}

#[derive(Args)]
pub struct ExportArgs {
    /// Output file (JSON format, stdout if omitted)
    #[arg(short, long)]
    pub output: Option<PathBuf>,

    /// Export all projects
    #[arg(long)]
    pub all_projects: bool,
}

/// Export format matching the knowledge graph structure
#[derive(Debug, Serialize, Deserialize)]
pub struct ExportData {
    pub version: String,
    pub projects: Vec<ProjectExport>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ProjectExport {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub entities: Vec<EntityExport>,
    pub relations: Vec<RelationExport>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EntityExport {
    pub name: String,
    #[serde(rename = "entityType")]
    pub entity_type: String,
    pub observations: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RelationExport {
    pub from: String,
    pub to: String,
    #[serde(rename = "relationType")]
    pub relation_type: String,
}

pub async fn run_import(args: &ImportArgs, _cli: &Cli, ctx: &AppContext) -> anyhow::Result<()> {
    tracing::info!("Importing from {:?}", args.file);

    // Read file
    let content = std::fs::read_to_string(&args.file)?;
    let data: ExportData = serde_json::from_str(&content)?;

    tracing::debug!("Import format version: {}", data.version);

    let mut total_entities = 0;
    let mut total_relations = 0;

    for project_data in data.projects {
        let project_name = args.target_project.as_deref().unwrap_or(&project_data.name);

        // Get or create project
        let project = if let Some(existing) = ctx.storage.get_project(project_name).await? {
            if !args.merge {
                let entity_count = ctx.storage.get_all_entities(&existing.id).await?.len();
                if entity_count > 0 {
                    anyhow::bail!(
                        "Project '{}' already has {} entities. Use --merge to add to existing data.",
                        project_name,
                        entity_count
                    );
                }
            }
            existing
        } else {
            let mut p = Project::new(project_name);
            if let Some(desc) = &project_data.description {
                p = p.with_description(desc);
            }
            ctx.storage.save_project(&p).await?;
            p
        };

        // Import entities
        for entity_data in &project_data.entities {
            let mut entity = Entity::new(
                project.id.clone(),
                &entity_data.name,
                &entity_data.entity_type,
            );
            for obs in &entity_data.observations {
                entity.add_observation(obs);
            }
            for tag in &entity_data.tags {
                entity.add_tag(tag);
            }
            ctx.storage.save_entity(&entity).await?;
            total_entities += 1;
        }

        // Import relations
        for rel_data in &project_data.relations {
            let relation = Relation::from_names(
                project.id.clone(),
                &rel_data.from,
                &rel_data.to,
                &rel_data.relation_type,
            );
            ctx.storage.save_relation(&relation).await?;
            total_relations += 1;
        }

        tracing::info!(
            "Imported {} entities and {} relations into project '{}'",
            project_data.entities.len(),
            project_data.relations.len(),
            project_name
        );
    }

    println!(
        "Imported {} entities and {} relations from {:?}",
        total_entities, total_relations, args.file
    );

    Ok(())
}

pub async fn run_export(args: &ExportArgs, cli: &Cli, ctx: &AppContext) -> anyhow::Result<()> {
    tracing::info!("Exporting data");

    let projects = if args.all_projects {
        ctx.storage.get_all_projects().await?
    } else {
        match ctx.storage.get_project(&cli.project).await? {
            Some(p) => vec![p],
            None => {
                println!("Project '{}' not found", cli.project);
                return Ok(());
            }
        }
    };

    let mut project_exports = Vec::new();

    for project in projects {
        let entities = ctx.storage.get_all_entities(&project.id).await?;
        let relations = ctx.storage.get_all_relations(&project.id).await?;

        let entity_exports: Vec<EntityExport> = entities
            .iter()
            .map(|e| EntityExport {
                name: e.name.clone(),
                entity_type: e.entity_type.0.clone(),
                observations: e.observations.iter().map(|o| o.content.clone()).collect(),
                tags: e.tags.clone(),
            })
            .collect();

        let relation_exports: Vec<RelationExport> = relations
            .iter()
            .map(|r| RelationExport {
                from: r.from_name.clone(),
                to: r.to_name.clone(),
                relation_type: r.relation_type.clone(),
            })
            .collect();

        tracing::debug!(
            "Exporting project '{}': {} entities, {} relations",
            project.name,
            entity_exports.len(),
            relation_exports.len()
        );

        project_exports.push(ProjectExport {
            name: project.name,
            description: project.description,
            entities: entity_exports,
            relations: relation_exports,
        });
    }

    let export_data = ExportData {
        version: "1.0".to_string(),
        projects: project_exports,
    };

    let json = serde_json::to_string_pretty(&export_data)?;

    if let Some(ref path) = args.output {
        // Write with secure permissions (0o600 = owner read/write only)
        #[cfg(unix)]
        {
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .mode(0o600)
                .open(path)?;
            file.write_all(json.as_bytes())?;
        }
        #[cfg(not(unix))]
        {
            std::fs::write(path, &json)?;
        }
        println!("Exported to {:?}", path);
    } else {
        println!("{}", json);
    }

    Ok(())
}
