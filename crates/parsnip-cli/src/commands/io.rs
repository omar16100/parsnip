//! Import/Export commands

use std::io::Write;
use std::path::PathBuf;

#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;

use clap::{Args, ValueEnum};
use serde::{Deserialize, Serialize};

use crate::{AppContext, Cli};
use parsnip_core::{Entity, Project, Relation};
use parsnip_storage::StorageBackend;

/// Export format
#[derive(Clone, Copy, Default, ValueEnum)]
pub enum ExportFormat {
    #[default]
    Json,
    Csv,
    #[value(name = "graphml")]
    GraphML,
}

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

    /// Import from knowledgegraph-mcp SQLite database
    #[arg(long)]
    pub from_knowledgegraph: bool,
}

#[derive(Args)]
pub struct ExportArgs {
    /// Output file (stdout if omitted)
    #[arg(short, long)]
    pub output: Option<PathBuf>,

    /// Export all projects
    #[arg(long)]
    pub all_projects: bool,

    /// Export format
    #[arg(short, long, default_value = "json")]
    pub format: ExportFormat,
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

    if args.from_knowledgegraph {
        #[cfg(feature = "migrate")]
        {
            return import_from_knowledgegraph(args, ctx).await;
        }
        #[cfg(not(feature = "migrate"))]
        {
            anyhow::bail!("Migration feature not enabled. Rebuild with --features migrate");
        }
    }

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

        // Build entities batch
        let entities: Vec<Entity> = project_data
            .entities
            .iter()
            .map(|entity_data| {
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
                entity
            })
            .collect();

        // Build relations batch
        let relations: Vec<Relation> = project_data
            .relations
            .iter()
            .map(|rel_data| {
                Relation::from_names(
                    project.id.clone(),
                    &rel_data.from,
                    &rel_data.to,
                    &rel_data.relation_type,
                )
            })
            .collect();

        // Batch save for efficiency
        ctx.storage.save_entities_batch(&entities).await?;
        ctx.storage.save_relations_batch(&relations).await?;
        total_entities += entities.len();
        total_relations += relations.len();

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

    let content = match args.format {
        ExportFormat::Json => serde_json::to_string_pretty(&export_data)?,
        ExportFormat::Csv => export_to_csv(&export_data),
        ExportFormat::GraphML => export_to_graphml(&export_data),
    };

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
            file.write_all(content.as_bytes())?;
        }
        #[cfg(not(unix))]
        {
            std::fs::write(path, &content)?;
        }
        println!("Exported to {:?}", path);
    } else {
        println!("{}", content);
    }

    Ok(())
}

fn export_to_csv(data: &ExportData) -> String {
    let mut output = String::new();

    // Entities CSV
    output.push_str("# Entities\n");
    output.push_str("project,name,type,observations,tags\n");

    for project in &data.projects {
        for entity in &project.entities {
            let obs = entity.observations.join("; ");
            let tags = entity.tags.join("; ");
            output.push_str(&format!(
                "{},{},{},\"{}\",\"{}\"\n",
                csv_escape(&project.name),
                csv_escape(&entity.name),
                csv_escape(&entity.entity_type),
                csv_escape(&obs),
                csv_escape(&tags)
            ));
        }
    }

    output.push_str("\n# Relations\n");
    output.push_str("project,from,to,type\n");

    for project in &data.projects {
        for relation in &project.relations {
            output.push_str(&format!(
                "{},{},{},{}\n",
                csv_escape(&project.name),
                csv_escape(&relation.from),
                csv_escape(&relation.to),
                csv_escape(&relation.relation_type)
            ));
        }
    }

    output
}

/// Escape a string for CSV output with formula injection protection
fn csv_escape(s: &str) -> String {
    // Protect against CSV formula injection (OWASP)
    // Prefix dangerous chars with ' to prevent spreadsheet interpretation
    let needs_formula_protection = s
        .chars()
        .next()
        .map(|c| matches!(c, '=' | '+' | '-' | '@' | '\t' | '\r'))
        .unwrap_or(false);

    let escaped = if needs_formula_protection {
        format!("'{}", s)
    } else {
        s.to_string()
    };

    // Quote if contains special CSV chars
    if escaped.contains(',') || escaped.contains('"') || escaped.contains('\n') {
        format!("\"{}\"", escaped.replace('"', "\"\""))
    } else {
        escaped
    }
}

fn export_to_graphml(data: &ExportData) -> String {
    let mut xml = String::new();

    xml.push_str(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<graphml xmlns="http://graphml.graphdrawing.org/xmlns"
         xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
         xsi:schemaLocation="http://graphml.graphdrawing.org/xmlns
         http://graphml.graphdrawing.org/xmlns/1.0/graphml.xsd">
  <key id="d0" for="node" attr.name="type" attr.type="string"/>
  <key id="d1" for="node" attr.name="observations" attr.type="string"/>
  <key id="d2" for="node" attr.name="tags" attr.type="string"/>
  <key id="d3" for="edge" attr.name="type" attr.type="string"/>
"#,
    );

    for project in &data.projects {
        xml.push_str(&format!(
            "  <graph id=\"{}\" edgedefault=\"directed\">\n",
            xml_escape(&project.name)
        ));

        // Nodes (entities)
        for entity in &project.entities {
            let obs = entity.observations.join("; ");
            let tags = entity.tags.join("; ");
            xml.push_str(&format!(
                "    <node id=\"{}\">\n      <data key=\"d0\">{}</data>\n      <data key=\"d1\">{}</data>\n      <data key=\"d2\">{}</data>\n    </node>\n",
                xml_escape(&entity.name),
                xml_escape(&entity.entity_type),
                xml_escape(&obs),
                xml_escape(&tags)
            ));
        }

        // Edges (relations)
        for (i, relation) in project.relations.iter().enumerate() {
            xml.push_str(&format!(
                "    <edge id=\"e{}\" source=\"{}\" target=\"{}\">\n      <data key=\"d3\">{}</data>\n    </edge>\n",
                i,
                xml_escape(&relation.from),
                xml_escape(&relation.to),
                xml_escape(&relation.relation_type)
            ));
        }

        xml.push_str("  </graph>\n");
    }

    xml.push_str("</graphml>\n");
    xml
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

/// Import from knowledgegraph-mcp SQLite database
#[cfg(feature = "migrate")]
async fn import_from_knowledgegraph(args: &ImportArgs, ctx: &AppContext) -> anyhow::Result<()> {
    use rusqlite::Connection;

    let db_path = &args.file;
    tracing::info!("Importing from knowledgegraph-mcp database: {:?}", db_path);

    let conn = Connection::open(db_path)?;

    // Get project name
    let project_name = args.target_project.as_deref().unwrap_or("default");
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
        let p = Project::new(project_name);
        ctx.storage.save_project(&p).await?;
        p
    };

    // Read entities from knowledgegraph-mcp into batch
    let mut stmt = conn.prepare("SELECT name, entity_type, observations, tags FROM entities")?;

    let entities_iter = stmt.query_map([], |row| {
        let name: String = row.get(0)?;
        let entity_type: String = row.get(1)?;
        let observations_json: String = row.get(2)?;
        let tags_json: Option<String> = row.get(3)?;
        Ok((name, entity_type, observations_json, tags_json))
    })?;

    let mut entities = Vec::new();
    for entity_result in entities_iter {
        let (name, entity_type, observations_json, tags_json) = entity_result?;

        let observations: Vec<String> =
            serde_json::from_str(&observations_json).unwrap_or_default();
        let tags: Vec<String> = tags_json
            .map(|t| serde_json::from_str(&t).unwrap_or_default())
            .unwrap_or_default();

        let mut entity = Entity::new(project.id.clone(), &name, &entity_type);
        for obs in observations {
            entity.add_observation(&obs);
        }
        for tag in tags {
            entity.add_tag(&tag);
        }
        entities.push(entity);
    }

    // Read relations into batch
    let mut stmt = conn.prepare("SELECT from_entity, to_entity, relation_type FROM relations")?;

    let relations_iter = stmt.query_map([], |row| {
        let from: String = row.get(0)?;
        let to: String = row.get(1)?;
        let rel_type: String = row.get(2)?;
        Ok((from, to, rel_type))
    })?;

    let mut relations = Vec::new();
    for rel_result in relations_iter {
        let (from, to, rel_type) = rel_result?;
        let relation = Relation::from_names(project.id.clone(), &from, &to, &rel_type);
        relations.push(relation);
    }

    // Batch save for efficiency
    let entity_count = entities.len();
    let relation_count = relations.len();
    ctx.storage.save_entities_batch(&entities).await?;
    ctx.storage.save_relations_batch(&relations).await?;

    tracing::info!(
        "Imported {} entities and {} relations from knowledgegraph-mcp",
        entity_count,
        relation_count
    );
    println!(
        "Imported {} entities and {} relations from knowledgegraph-mcp into project '{}'",
        entity_count, relation_count, project_name
    );

    Ok(())
}
