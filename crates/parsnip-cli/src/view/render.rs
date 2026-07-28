//! Rendering views as table, JSON or CSV.
//!
//! Table output is hand-formatted rather than produced by a table crate: the shapes here
//! are small and mostly indented lists rather than grids, and AGENTS.md asks for a
//! justification before adding a dependency.

use std::io::{self, Write};

use serde::Serialize;

use super::*;

/// Output format, shared by every command.
///
/// `Graphml` is only meaningful for `export`; other commands reject it rather than
/// pretending to support it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, clap::ValueEnum)]
pub enum OutputFormat {
    #[default]
    Table,
    Json,
    Csv,
    #[value(name = "graphml")]
    Graphml,
}

impl OutputFormat {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Table => "table",
            Self::Json => "json",
            Self::Csv => "csv",
            Self::Graphml => "graphml",
        }
    }
}

/// A view that can be rendered.
///
/// `csv` returns `None` for shapes that are not rows: an entity's detail, a traversal, or
/// the result of a write. Forcing those into CSV would produce something misleading.
pub trait Render: Serialize {
    fn table(&self, w: &mut dyn Write) -> io::Result<()>;

    fn csv(&self, _w: &mut dyn Write) -> Option<io::Result<()>> {
        None
    }
}

/// Render a view to stdout in the requested format.
///
/// Exits with status 2 when the format cannot represent the view, rather than emitting
/// something that looks like data but is not.
pub fn emit<V: Render>(view: &V, format: OutputFormat) -> anyhow::Result<()> {
    let stdout = io::stdout();
    let mut out = stdout.lock();

    match format {
        OutputFormat::Table => view.table(&mut out)?,
        OutputFormat::Json => {
            serde_json::to_writer_pretty(&mut out, view)?;
            writeln!(out)?;
        }
        OutputFormat::Csv => match view.csv(&mut out) {
            Some(result) => result?,
            None => {
                eprintln!(
                    "csv output is not supported for this command; it has no row shape. \
                     Use --format json."
                );
                std::process::exit(2);
            }
        },
        OutputFormat::Graphml => {
            eprintln!("graphml output is only supported by `parsnip export`.");
            std::process::exit(2);
        }
    }

    Ok(())
}

/// Quote a CSV field.
///
/// Leading `=`, `+`, `-` and `@` are prefixed with a quote so spreadsheet software does not
/// evaluate the value as a formula. This mirrors the escaping the export command already
/// applies (OWASP CSV injection).
fn csv_field(value: &str) -> String {
    let needs_formula_guard = value.starts_with(['=', '+', '-', '@']);
    let guarded = if needs_formula_guard {
        format!("'{value}")
    } else {
        value.to_string()
    };

    if guarded.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", guarded.replace('"', "\"\""))
    } else {
        guarded
    }
}

fn csv_row(w: &mut dyn Write, fields: &[&str]) -> io::Result<()> {
    let line: Vec<String> = fields.iter().map(|f| csv_field(f)).collect();
    writeln!(w, "{}", line.join(","))
}

/// `[tag, tag]` suffix, or nothing when there are no tags.
fn tag_suffix(tags: &[String]) -> String {
    if tags.is_empty() {
        String::new()
    } else {
        format!(" [{}]", tags.join(", "))
    }
}

fn weight_suffix(weight: Option<f64>) -> String {
    weight
        .map(|w| format!(" (weight: {:.2})", w))
        .unwrap_or_default()
}

impl Render for EntityListView {
    fn table(&self, w: &mut dyn Write) -> io::Result<()> {
        if self.entities.is_empty() {
            return writeln!(w, "No entities found in project '{}'", self.project);
        }
        writeln!(
            w,
            "Entities in project '{}' ({} found):",
            self.project,
            self.entities.len()
        )?;
        for e in &self.entities {
            writeln!(w, "  {} ({}){}", e.name, e.entity_type, tag_suffix(&e.tags))?;
        }
        Ok(())
    }

    fn csv(&self, w: &mut dyn Write) -> Option<io::Result<()>> {
        Some((|| {
            csv_row(w, &["project", "name", "type", "tags"])?;
            for e in &self.entities {
                csv_row(
                    w,
                    &[&self.project, &e.name, &e.entity_type, &e.tags.join(";")],
                )?;
            }
            Ok(())
        })())
    }
}

impl Render for EntityDetailView {
    fn table(&self, w: &mut dyn Write) -> io::Result<()> {
        writeln!(w, "Entity: {}", self.name)?;
        writeln!(w, "  Type: {}", self.entity_type)?;
        writeln!(w, "  Project: {}", self.project)?;
        writeln!(w, "  Created: {}", self.created_at)?;
        writeln!(w, "  Updated: {}", self.updated_at)?;
        if !self.tags.is_empty() {
            writeln!(w, "  Tags: {}", self.tags.join(", "))?;
        }
        if !self.observations.is_empty() {
            writeln!(w, "  Observations:")?;
            for o in &self.observations {
                writeln!(w, "    - {} ({})", o.content, o.created_at)?;
            }
        }
        Ok(())
    }
}

impl Render for RelationListView {
    fn table(&self, w: &mut dyn Write) -> io::Result<()> {
        if self.relations.is_empty() {
            return writeln!(w, "No relations found in project '{}'", self.project);
        }
        writeln!(
            w,
            "Relations in project '{}' ({} found):",
            self.project,
            self.relations.len()
        )?;
        for r in &self.relations {
            writeln!(
                w,
                "  {} -[{}]-> {}{}",
                r.from,
                r.relation_type,
                r.to,
                weight_suffix(r.weight)
            )?;
        }
        Ok(())
    }

    fn csv(&self, w: &mut dyn Write) -> Option<io::Result<()>> {
        Some((|| {
            csv_row(w, &["project", "from", "to", "type", "weight"])?;
            for r in &self.relations {
                let weight = r.weight.map(|x| x.to_string()).unwrap_or_default();
                csv_row(
                    w,
                    &[&self.project, &r.from, &r.to, &r.relation_type, &weight],
                )?;
            }
            Ok(())
        })())
    }
}

impl Render for TraversalView {
    fn table(&self, w: &mut dyn Write) -> io::Result<()> {
        writeln!(
            w,
            "Traversal from '{}' (depth: {}, direction: {}):",
            self.start, self.depth, self.direction
        )?;
        writeln!(
            w,
            "  Visited {} entities, traversed {} edges",
            self.nodes_visited, self.edges_traversed
        )?;
        // Matches the original: one visited entity means the start node only.
        if self.entities.len() <= 1 {
            return writeln!(w, "  (no connected entities found)");
        }
        writeln!(w, "  Entities: {}", self.entities.join(", "))?;
        if !self.relations.is_empty() {
            writeln!(w, "  Relations:")?;
            for r in &self.relations {
                writeln!(
                    w,
                    "    {} -[{}]-> {}{}",
                    r.from,
                    r.relation_type,
                    r.to,
                    weight_suffix(r.weight)
                )?;
            }
        }
        Ok(())
    }
}

impl Render for FindPathView {
    fn table(&self, w: &mut dyn Write) -> io::Result<()> {
        if self.paths.is_empty() {
            writeln!(w, "No path found from '{}' to '{}'", self.from, self.to)?;
            return writeln!(
                w,
                "  (searched {} nodes, {} edges)",
                self.nodes_visited, self.edges_traversed
            );
        }

        writeln!(
            w,
            "Path found from '{}' to '{}' using {}:",
            self.from, self.to, self.algorithm
        )?;

        for (i, path) in self.paths.iter().enumerate() {
            writeln!(
                w,
                "\n  Path {}: {} hops, total weight: {:.2}",
                i + 1,
                path.length,
                path.total_weight
            )?;
            writeln!(w, "  Route: {}", path.nodes.join(" -> "))?;
            for edge in &path.edges {
                writeln!(
                    w,
                    "    {} -[{}]-> {}{}",
                    edge.from,
                    edge.relation_type,
                    edge.to,
                    weight_suffix(edge.weight)
                )?;
            }
        }

        writeln!(
            w,
            "\n  Stats: visited {} nodes, traversed {} edges",
            self.nodes_visited, self.edges_traversed
        )
    }
}

impl Render for ProjectListView {
    fn table(&self, w: &mut dyn Write) -> io::Result<()> {
        if self.projects.is_empty() {
            return writeln!(
                w,
                "No projects found. Create one with 'parsnip project create <name>'"
            );
        }
        writeln!(w, "Projects ({} found):", self.projects.len())?;
        for p in &self.projects {
            let current = if p.current { " (current)" } else { "" };
            let desc = p
                .description
                .as_ref()
                .map(|d| format!(" - {}", d))
                .unwrap_or_default();
            writeln!(w, "  {}{}{}", p.name, current, desc)?;
        }
        Ok(())
    }

    fn csv(&self, w: &mut dyn Write) -> Option<io::Result<()>> {
        Some((|| {
            csv_row(w, &["name", "description", "current"])?;
            for p in &self.projects {
                csv_row(
                    w,
                    &[
                        &p.name,
                        p.description.as_deref().unwrap_or(""),
                        if p.current { "true" } else { "false" },
                    ],
                )?;
            }
            Ok(())
        })())
    }
}

impl Render for ProjectStatsView {
    fn table(&self, w: &mut dyn Write) -> io::Result<()> {
        writeln!(w, "Stats for project '{}':", self.project)?;
        if let Some(d) = &self.description {
            writeln!(w, "  Description: {}", d)?;
        }
        writeln!(w, "  Created: {}", self.created_at)?;
        writeln!(w)?;
        writeln!(w, "  Entities: {}", self.entity_count)?;
        for row in &self.entities_by_type {
            writeln!(w, "    {}: {}", row.name, row.count)?;
        }
        writeln!(w)?;
        writeln!(w, "  Observations: {}", self.observation_count)?;
        writeln!(w, "  Tags: {}", self.tag_count)?;
        writeln!(w)?;
        writeln!(w, "  Relations: {}", self.relation_count)?;
        for row in &self.relations_by_type {
            writeln!(w, "    {}: {}", row.name, row.count)?;
        }
        Ok(())
    }
}

impl Render for SearchView {
    fn table(&self, w: &mut dyn Write) -> io::Result<()> {
        if self.results.is_empty() {
            return writeln!(w, "No results found in {}", self.scope);
        }

        match &self.query {
            Some(q) => writeln!(
                w,
                "Search results for '{}' in {} ({} found):",
                q,
                self.scope,
                self.results.len()
            )?,
            None => writeln!(
                w,
                "Search results for tags {:?} in {} ({} found):",
                self.tags,
                self.scope,
                self.results.len()
            )?,
        }

        for hit in &self.results {
            writeln!(
                w,
                "  {} ({}){}",
                hit.name,
                hit.entity_type,
                tag_suffix(&hit.tags)
            )?;
            for rel in &hit.relations {
                writeln!(
                    w,
                    "    {} {} ({})",
                    rel.direction, rel.other, rel.relation_type
                )?;
            }
        }
        Ok(())
    }

    fn csv(&self, w: &mut dyn Write) -> Option<io::Result<()>> {
        Some((|| {
            csv_row(w, &["name", "type", "tags"])?;
            for hit in &self.results {
                csv_row(w, &[&hit.name, &hit.entity_type, &hit.tags.join(";")])?;
            }
            Ok(())
        })())
    }
}

impl Render for MutationView {
    fn table(&self, w: &mut dyn Write) -> io::Result<()> {
        writeln!(w, "{}", self.message)?;
        for d in &self.details {
            writeln!(w, "{}", d)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn render_table<V: Render>(v: &V) -> String {
        let mut buf = Vec::new();
        v.table(&mut buf).unwrap();
        String::from_utf8(buf).unwrap()
    }

    fn render_csv<V: Render>(v: &V) -> Option<String> {
        let mut buf = Vec::new();
        v.csv(&mut buf)?.unwrap();
        Some(String::from_utf8(buf).unwrap())
    }

    fn entity_list() -> EntityListView {
        EntityListView {
            project: "default".into(),
            entities: vec![
                EntityRow {
                    name: "alice".into(),
                    entity_type: "person".into(),
                    tags: vec!["dev".into(), "core".into()],
                },
                EntityRow {
                    name: "widget".into(),
                    entity_type: "thing".into(),
                    tags: vec![],
                },
            ],
        }
    }

    /// Pins the exact table text, which is what stops the render refactor from quietly
    /// reformatting output that people script against.
    #[test]
    fn entity_list_table_matches_the_original_format() {
        assert_eq!(
            render_table(&entity_list()),
            "Entities in project 'default' (2 found):\n  alice (person) [dev, core]\n  widget (thing)\n"
        );
    }

    #[test]
    fn empty_entity_list_says_so() {
        let view = EntityListView {
            project: "default".into(),
            entities: vec![],
        };
        assert_eq!(
            render_table(&view),
            "No entities found in project 'default'\n"
        );
    }

    #[test]
    fn relation_table_shows_weight_only_when_present() {
        let view = RelationListView {
            project: "default".into(),
            relations: vec![
                RelationRow {
                    from: "alice".into(),
                    to: "bob".into(),
                    relation_type: "knows".into(),
                    weight: Some(0.8),
                },
                RelationRow {
                    from: "bob".into(),
                    to: "widget".into(),
                    relation_type: "owns".into(),
                    weight: None,
                },
            ],
        };
        assert_eq!(
            render_table(&view),
            "Relations in project 'default' (2 found):\n  \
             alice -[knows]-> bob (weight: 0.80)\n  bob -[owns]-> widget\n"
        );
    }

    #[test]
    fn csv_has_a_header_and_one_row_per_entity() {
        let csv = render_csv(&entity_list()).expect("entity list is row-shaped");
        assert_eq!(
            csv,
            "project,name,type,tags\ndefault,alice,person,dev;core\ndefault,widget,thing,\n"
        );
    }

    #[test]
    fn csv_is_refused_for_shapes_that_are_not_rows() {
        let detail = EntityDetailView {
            name: "alice".into(),
            entity_type: "person".into(),
            project: "default".into(),
            created_at: "now".into(),
            updated_at: "now".into(),
            tags: vec![],
            observations: vec![],
        };
        assert!(
            render_csv(&detail).is_none(),
            "an entity detail has no row shape and must not pretend to"
        );
        assert!(render_csv(&MutationView::new("done")).is_none());
    }

    #[test]
    fn csv_quotes_separators_and_guards_formulas() {
        let view = EntityListView {
            project: "p".into(),
            entities: vec![
                EntityRow {
                    name: "has,comma".into(),
                    entity_type: "t".into(),
                    tags: vec![],
                },
                EntityRow {
                    name: "=cmd()".into(),
                    entity_type: "t".into(),
                    tags: vec![],
                },
                EntityRow {
                    name: "quote\"inside".into(),
                    entity_type: "t".into(),
                    tags: vec![],
                },
            ],
        };
        let csv = render_csv(&view).unwrap();
        assert!(csv.contains("\"has,comma\""), "{csv}");
        assert!(
            csv.contains("'=cmd()"),
            "formula must be neutralised: {csv}"
        );
        assert!(csv.contains("\"quote\"\"inside\""), "{csv}");
    }

    #[test]
    fn json_uses_camel_case_keys() {
        let json = serde_json::to_value(entity_list()).unwrap();
        assert_eq!(json["entities"][0]["entityType"], "person");
        assert_eq!(json["entities"][0]["tags"][0], "dev");
    }

    #[test]
    fn search_table_renders_both_query_and_tag_headings() {
        let hit = || SearchHit {
            name: "alice".into(),
            entity_type: "person".into(),
            tags: vec!["dev".into()],
            relations: vec![],
        };

        let by_query = SearchView {
            query: Some("likes".into()),
            tags: vec![],
            scope: "default".into(),
            results: vec![hit()],
        };
        assert!(
            render_table(&by_query).starts_with("Search results for 'likes' in default (1 found):")
        );

        let by_tag = SearchView {
            query: None,
            tags: vec!["dev".into()],
            scope: "default".into(),
            results: vec![hit()],
        };
        assert!(
            render_table(&by_tag)
                .starts_with("Search results for tags [\"dev\"] in default (1 found):"),
            "{}",
            render_table(&by_tag)
        );
    }
}
