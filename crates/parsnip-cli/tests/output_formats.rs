//! Output format tests.
//!
//! Two jobs. First, pin table output: the render refactor moved every command from inline
//! `println!` to a view, and people script against that text. Second, prove `--format json`
//! and `--format csv` actually work, which they never did before: `output.rs` was dead code
//! with "not yet implemented" stubs, so the flag was parsed and ignored everywhere.

use std::process::Command;

use assert_cmd::cargo::cargo_bin;
use tempfile::TempDir;

fn parsnip(dir: &std::path::Path) -> Command {
    let mut cmd = Command::new(cargo_bin("parsnip"));
    cmd.arg("--data-dir").arg(dir).arg("--local");
    cmd.env_remove("PARSNIP_SERVER")
        .env_remove("PARSNIP_AUTH_TOKEN");
    cmd
}

fn run(dir: &std::path::Path, args: &[&str]) -> String {
    let out = parsnip(dir).args(args).output().expect("run parsnip");
    assert!(
        out.status.success(),
        "`parsnip {}` failed: {}",
        args.join(" "),
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).into_owned()
}

/// A fixture with two entities, tags, observations, a weighted relation and a plain one.
fn fixture() -> TempDir {
    let dir = TempDir::new().unwrap();
    let p = dir.path();
    run(
        p,
        &[
            "entity",
            "add",
            "alice",
            "--type",
            "person",
            "--obs",
            "likes rust",
            "--tag",
            "dev",
            "--tag",
            "core",
        ],
    );
    run(
        p,
        &[
            "entity", "add", "bob", "--type", "person", "--obs", "likes go", "--tag", "dev",
        ],
    );
    run(p, &["entity", "add", "widget", "--type", "thing"]);
    run(
        p,
        &[
            "relation", "add", "alice", "bob", "--type", "knows", "--weight", "0.8",
        ],
    );
    run(p, &["relation", "add", "bob", "widget", "--type", "owns"]);
    dir
}

// ── Table output, pinned exactly ────────────────────────────────────────────────

#[test]
fn entity_list_table_is_unchanged() {
    let dir = fixture();
    assert_eq!(
        run(dir.path(), &["entity", "list"]),
        "Entities in project 'default' (3 found):\n  \
         alice (person) [dev, core]\n  bob (person) [dev]\n  widget (thing)\n"
    );
}

#[test]
fn relation_list_table_is_unchanged() {
    let dir = fixture();
    assert_eq!(
        run(dir.path(), &["relation", "list"]),
        "Relations in project 'default' (2 found):\n  \
         alice -[knows]-> bob (weight: 0.80)\n  bob -[owns]-> widget\n"
    );
}

#[test]
fn entity_get_table_is_unchanged_except_timestamps() {
    let dir = fixture();
    let out = run(dir.path(), &["entity", "get", "alice"]);
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines[0], "Entity: alice");
    assert_eq!(lines[1], "  Type: person");
    assert_eq!(lines[2], "  Project: default");
    assert!(lines[3].starts_with("  Created: "));
    assert!(lines[4].starts_with("  Updated: "));
    assert_eq!(lines[5], "  Tags: dev, core");
    assert_eq!(lines[6], "  Observations:");
    assert!(lines[7].starts_with("    - likes rust ("));
}

#[test]
fn search_table_is_unchanged() {
    let dir = fixture();
    assert_eq!(
        run(dir.path(), &["search", "likes"]),
        "Search results for 'likes' in default (2 found):\n  \
         alice (person) [dev, core]\n  bob (person) [dev]\n"
    );
}

/// Breakdown lines used to come out of HashMaps, so two runs over identical data printed
/// them in different orders. They are sorted now.
#[test]
fn stats_breakdown_is_deterministic() {
    let dir = fixture();
    let first = run(dir.path(), &["project", "stats"]);
    for _ in 0..4 {
        assert_eq!(
            run(dir.path(), &["project", "stats"]),
            first,
            "project stats must not vary between runs"
        );
    }
    assert!(first.contains("    person: 2\n    thing: 1"), "{first}");
    assert!(first.contains("    knows: 1\n    owns: 1"), "{first}");
}

#[test]
fn traversal_entity_order_is_deterministic() {
    let dir = fixture();
    let first = run(dir.path(), &["relation", "traverse", "alice"]);
    for _ in 0..4 {
        assert_eq!(run(dir.path(), &["relation", "traverse", "alice"]), first);
    }
    assert!(first.contains("  Entities: alice, bob, widget"), "{first}");
}

// ── JSON ────────────────────────────────────────────────────────────────────────

fn json(dir: &std::path::Path, args: &[&str]) -> serde_json::Value {
    let mut full = vec!["--format", "json"];
    full.extend_from_slice(args);
    let out = run(dir, &full);
    serde_json::from_str(&out)
        .unwrap_or_else(|e| panic!("`parsnip {}` did not emit JSON: {e}\n{out}", args.join(" ")))
}

#[test]
fn every_read_command_emits_valid_json() {
    let dir = fixture();
    let p = dir.path();
    for args in [
        vec!["entity", "list"],
        vec!["entity", "get", "alice"],
        vec!["relation", "list"],
        vec!["relation", "traverse", "alice"],
        vec!["relation", "find-path", "alice", "widget"],
        vec!["project", "list"],
        vec!["project", "stats"],
        vec!["search", "likes"],
    ] {
        json(p, &args);
    }
}

#[test]
fn json_carries_the_fields_the_table_shows() {
    let dir = fixture();
    let p = dir.path();

    let list = json(p, &["entity", "list"]);
    assert_eq!(list["entities"][0]["name"], "alice");
    assert_eq!(list["entities"][0]["entityType"], "person");
    assert_eq!(list["entities"][0]["tags"][1], "core");

    let rels = json(p, &["relation", "list"]);
    let knows = rels["relations"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["relationType"] == "knows")
        .expect("knows relation present");
    assert_eq!(knows["weight"], 0.8, "weight must survive into JSON");

    let stats = json(p, &["project", "stats"]);
    assert_eq!(stats["entityCount"], 3);
    assert_eq!(stats["relationCount"], 2);
    assert_eq!(stats["entitiesByType"][0]["name"], "person");
    assert_eq!(stats["entitiesByType"][0]["count"], 2);

    let detail = json(p, &["entity", "get", "alice"]);
    assert_eq!(detail["observations"][0]["content"], "likes rust");
}

#[test]
fn mutations_emit_json_too() {
    let dir = fixture();
    let created = json(dir.path(), &["entity", "add", "carol", "--type", "person"]);
    assert_eq!(created["message"], "Created entity: carol (type: person)");
}

// ── CSV ─────────────────────────────────────────────────────────────────────────

#[test]
fn csv_is_produced_for_row_shaped_commands() {
    let dir = fixture();
    assert_eq!(
        run(dir.path(), &["--format", "csv", "entity", "list"]),
        "project,name,type,tags\n\
         default,alice,person,dev;core\n\
         default,bob,person,dev\n\
         default,widget,thing,\n"
    );

    let rels = run(dir.path(), &["--format", "csv", "relation", "list"]);
    assert!(rels.starts_with("project,from,to,type,weight\n"), "{rels}");
    assert!(rels.contains("default,alice,bob,knows,0.8"), "{rels}");
}

#[test]
fn csv_is_refused_rather_than_faked_for_detail_shapes() {
    let dir = fixture();
    let out = parsnip(dir.path())
        .args(["--format", "csv", "entity", "get", "alice"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&out.stderr).contains("not supported"));
}

#[test]
fn graphml_is_export_only() {
    let dir = fixture();
    let out = parsnip(dir.path())
        .args(["--format", "graphml", "entity", "list"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&out.stderr).contains("export"));
}

// ── export keeps its own behaviour on the shared flag ───────────────────────────

#[test]
fn export_defaults_to_json_and_honours_the_shared_format_flag() {
    let dir = fixture();
    let p = dir.path();

    // No flag: still JSON, as it has always been, even though the global default is table.
    let default = run(p, &["export"]);
    assert!(default.trim_start().starts_with('{'), "{default}");
    serde_json::from_str::<serde_json::Value>(&default).expect("export default is JSON");

    let csv = run(p, &["--format", "csv", "export"]);
    assert!(csv.starts_with("# Entities\n"), "{csv}");

    let graphml = run(p, &["--format", "graphml", "export"]);
    assert!(graphml.starts_with("<?xml "), "{graphml}");
}
