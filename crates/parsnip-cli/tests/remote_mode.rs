//! End-to-end tests: a real daemon process, the real CLI binary, real HTTP.
//!
//! The acceptance criterion for this feature lives here: with a daemon holding the redb
//! lock, the local CLI must fail and the same command via `--server` must succeed.

use std::net::TcpListener;
use std::process::{Child, Command};
use std::time::{Duration, Instant};

use tempfile::TempDir;

/// A `parsnip serve` child that is killed when the test ends, however it ends.
struct Daemon {
    child: Child,
    url: String,
}

impl Drop for Daemon {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn parsnip() -> Command {
    Command::new(env!("CARGO_BIN_EXE_parsnip"))
}

/// Reserve a port by binding and immediately releasing it.
///
/// Racy in principle, but the daemon binds within milliseconds and the alternative
/// (`--port 0` plus scraping the log) is more fragile.
fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .expect("bind ephemeral port")
        .local_addr()
        .expect("local addr")
        .port()
}

fn start_daemon(data_dir: &std::path::Path, token: Option<&str>) -> Daemon {
    let port = free_port();
    let mut cmd = parsnip();
    cmd.arg("--data-dir")
        .arg(data_dir)
        .arg("--local")
        .env_remove("PARSNIP_SERVER")
        .env_remove("PARSNIP_AUTH_TOKEN");
    if let Some(t) = token {
        cmd.arg("--auth-token").arg(t);
    }
    cmd.arg("serve")
        .arg("--transport")
        .arg("sse")
        .arg("--port")
        .arg(port.to_string())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());

    // Wrapped in `Daemon` straight away so its Drop kills and reaps the child on every
    // path, including the panic below when the daemon never becomes healthy.
    let daemon = Daemon {
        child: cmd.spawn().expect("spawn parsnip serve"),
        url: format!("http://127.0.0.1:{port}"),
    };

    // Poll /health until the daemon answers.
    let deadline = Instant::now() + Duration::from_secs(20);
    let health = format!("{}/health", daemon.url);
    while Instant::now() < deadline {
        let probe = Command::new("curl")
            .args(["-sf", "-m", "1", &health])
            .output();
        if matches!(probe, Ok(o) if o.status.success()) {
            return daemon;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    panic!("daemon at {} never became healthy", daemon.url);
}

/// Run the CLI against a daemon.
fn remote(url: &str) -> Command {
    let mut cmd = parsnip();
    cmd.arg("--server").arg(url);
    cmd.env_remove("PARSNIP_SERVER")
        .env_remove("PARSNIP_AUTH_TOKEN");
    cmd
}

/// Run the CLI against the local database.
fn local(data_dir: &std::path::Path) -> Command {
    let mut cmd = parsnip();
    cmd.arg("--data-dir").arg(data_dir).arg("--local");
    cmd.env_remove("PARSNIP_SERVER")
        .env_remove("PARSNIP_AUTH_TOKEN");
    cmd
}

fn stdout_of(cmd: &mut Command) -> String {
    let out = cmd.output().expect("run parsnip");
    assert!(
        out.status.success(),
        "command failed: {}\nstdout: {}\nstderr: {}",
        out.status,
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );
    String::from_utf8_lossy(&out.stdout).into_owned()
}

/// THE acceptance criterion for this feature.
#[test]
fn local_cli_is_locked_out_but_remote_works() {
    let dir = TempDir::new().unwrap();
    stdout_of(local(dir.path()).args([
        "entity", "add", "seed", "--type", "test", "--obs", "planted",
    ]));

    let daemon = start_daemon(dir.path(), None);

    // Local: the daemon owns the redb lock, so this must fail, and the message must
    // explain how to reach the daemon rather than just stating the lock exists.
    let out = local(dir.path()).args(["entity", "list"]).output().unwrap();
    assert!(
        !out.status.success(),
        "local access should fail while the daemon holds the lock"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("already open"),
        "expected a lock error, got: {stderr}"
    );
    assert!(
        stderr.contains("--server"),
        "the lock error must tell the user how to reach the daemon, got: {stderr}"
    );

    // Remote: the same command, same data, must work.
    let listed = stdout_of(remote(&daemon.url).args(["entity", "list"]));
    assert!(
        listed.contains("seed"),
        "remote listing missing the entity: {listed}"
    );
}

#[test]
fn full_command_surface_works_remotely() {
    let dir = TempDir::new().unwrap();
    let daemon = start_daemon(dir.path(), None);
    let url = &daemon.url;

    stdout_of(remote(url).args([
        "entity",
        "add",
        "alice",
        "--type",
        "person",
        "--obs",
        "likes rust",
        "--tag",
        "dev",
    ]));
    stdout_of(remote(url).args([
        "entity", "add", "bob", "--type", "person", "--obs", "likes go",
    ]));

    let got = stdout_of(remote(url).args(["entity", "get", "alice"]));
    assert!(got.contains("likes rust"), "{got}");
    assert!(got.contains("dev"), "tags must survive: {got}");
    // Timestamps have no lossless equivalent in the MCP tool surface; they must be here.
    assert!(got.contains("Created:"), "{got}");

    // Relation weights likewise have no MCP-tool equivalent.
    stdout_of(remote(url).args([
        "relation", "add", "alice", "bob", "--type", "knows", "--weight", "0.8",
    ]));
    let rels = stdout_of(remote(url).args(["relation", "list"]));
    assert!(
        rels.contains("weight: 0.80"),
        "weight must round-trip: {rels}"
    );

    let traversed = stdout_of(remote(url).args(["relation", "traverse", "alice", "--depth", "2"]));
    assert!(traversed.contains("bob"), "{traversed}");

    let stats = stdout_of(remote(url).args(["project", "stats"]));
    assert!(stats.contains("Entities: 2"), "{stats}");

    let projects = stdout_of(remote(url).args(["project", "list"]));
    assert!(projects.contains("default"), "{projects}");
}

#[test]
fn export_and_import_round_trip_remotely() {
    let dir = TempDir::new().unwrap();
    let daemon = start_daemon(dir.path(), None);
    let url = &daemon.url;

    stdout_of(remote(url).args([
        "entity", "add", "thing", "--type", "widget", "--obs", "exported",
    ]));

    let export_path = dir.path().join("backup.json");
    stdout_of(remote(url).args(["export", "-o"]).arg(&export_path));
    let exported = std::fs::read_to_string(&export_path).expect("export file written");
    assert!(exported.contains("thing"), "{exported}");

    // Import into a different project through the same daemon.
    stdout_of(remote(url).args(["import"]).arg(&export_path).args([
        "--target-project",
        "restored",
        "--merge",
    ]));
    let listed = stdout_of(remote(url).args(["-p", "restored", "entity", "list"]));
    assert!(listed.contains("thing"), "{listed}");
}

/// Local and remote must render identically, or the remote path has quietly lost a field.
#[test]
fn search_output_is_identical_local_and_remote() {
    let dir = TempDir::new().unwrap();
    for (name, obs) in [
        ("octopus", "eight arms very clever"),
        ("otter", "uses rocks as tools"),
        ("zebra", "stripey quadruped"),
    ] {
        stdout_of(
            local(dir.path()).args(["entity", "add", name, "--type", "animal", "--obs", obs]),
        );
    }

    // Capture local output first: the daemon cannot be running while we use the database.
    let mut local_outputs = Vec::new();
    for mode in ["exact", "fuzzy", "fulltext", "hybrid"] {
        local_outputs.push(stdout_of(
            local(dir.path()).args(["search", "rocks", "--mode", mode]),
        ));
    }

    let daemon = start_daemon(dir.path(), None);
    for (i, mode) in ["exact", "fuzzy", "fulltext", "hybrid"].iter().enumerate() {
        let remote_output =
            stdout_of(remote(&daemon.url).args(["search", "rocks", "--mode", mode]));
        assert_eq!(
            local_outputs[i], remote_output,
            "search --mode {mode} rendered differently locally and remotely"
        );
    }
}

#[test]
fn concurrent_remote_clients_all_succeed() {
    let dir = TempDir::new().unwrap();
    let daemon = start_daemon(dir.path(), None);
    let url = daemon.url.clone();

    let handles: Vec<_> = (0..4)
        .map(|i| {
            let url = url.clone();
            std::thread::spawn(move || {
                let out = remote(&url)
                    .args([
                        "entity",
                        "add",
                        &format!("concurrent_{i}"),
                        "--type",
                        "test",
                    ])
                    .output()
                    .expect("run parsnip");
                out.status.success()
            })
        })
        .collect();

    for h in handles {
        assert!(h.join().unwrap(), "a concurrent remote client failed");
    }

    let listed = stdout_of(remote(&url).args(["entity", "list"]));
    for i in 0..4 {
        assert!(listed.contains(&format!("concurrent_{i}")), "{listed}");
    }
}

/// Two clients creating the same new project at once must not orphan each other's data.
#[test]
fn concurrent_clients_agree_on_one_project() {
    let dir = TempDir::new().unwrap();
    let daemon = start_daemon(dir.path(), None);
    let url = daemon.url.clone();

    let handles: Vec<_> = (0..4)
        .map(|i| {
            let url = url.clone();
            std::thread::spawn(move || {
                remote(&url)
                    .args([
                        "-p",
                        "brandnew",
                        "entity",
                        "add",
                        &format!("e{i}"),
                        "--type",
                        "test",
                    ])
                    .output()
                    .expect("run parsnip")
                    .status
                    .success()
            })
        })
        .collect();
    for h in handles {
        assert!(h.join().unwrap());
    }

    // All four entities must be visible under the one project name. If the get-then-create
    // race were open, the losers' entities would be keyed under an orphaned project id.
    let listed = stdout_of(remote(&url).args(["-p", "brandnew", "entity", "list"]));
    for i in 0..4 {
        assert!(
            listed.contains(&format!("e{i}")),
            "entity e{i} was orphaned by a project id race: {listed}"
        );
    }

    let projects = stdout_of(remote(&url).args(["project", "list"]));
    assert_eq!(
        projects.matches("brandnew").count(),
        1,
        "exactly one project should exist: {projects}"
    );
}

#[test]
fn wrong_token_fails_clearly() {
    let dir = TempDir::new().unwrap();
    let daemon = start_daemon(dir.path(), Some("right-token"));

    let out = remote(&daemon.url)
        .args(["--auth-token", "wrong-token", "entity", "list"])
        .output()
        .unwrap();
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.to_lowercase().contains("token"),
        "error should name the token, got: {stderr}"
    );
}

#[test]
fn right_token_succeeds() {
    let dir = TempDir::new().unwrap();
    let daemon = start_daemon(dir.path(), Some("right-token"));
    stdout_of(remote(&daemon.url).args(["--auth-token", "right-token", "entity", "list"]));
}

#[test]
fn unreachable_daemon_fails_without_panicking() {
    let port = free_port(); // nothing is listening here
    let out = remote(&format!("http://127.0.0.1:{port}"))
        .args(["entity", "list"])
        .output()
        .unwrap();

    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        !stderr.contains("panicked"),
        "an unreachable daemon must not panic: {stderr}"
    );
    assert!(
        stderr.contains("cannot reach") || stderr.contains("daemon"),
        "error should say the daemon is unreachable, got: {stderr}"
    );
}

/// `--local` must override PARSNIP_SERVER, not be rejected as conflicting with it: the
/// variable is exported in the normal remote-mode setup, and `--local` is the documented
/// way to bypass the daemon for one command.
#[test]
fn local_flag_overrides_an_exported_server() {
    let dir = TempDir::new().unwrap();
    let unreachable = format!("http://127.0.0.1:{}", free_port());

    stdout_of(local(dir.path()).env("PARSNIP_SERVER", &unreachable).args([
        "entity",
        "add",
        "offline_only",
        "-t",
        "thing",
    ]));
    let listed = stdout_of(
        local(dir.path())
            .env("PARSNIP_SERVER", &unreachable)
            .args(["entity", "list"]),
    );
    assert!(
        listed.contains("offline_only"),
        "--local should have used the local database, got: {listed}"
    );
}

/// These commands never touch storage, so they must work while the database is locked.
#[test]
fn config_and_completions_work_while_the_database_is_locked() {
    let dir = TempDir::new().unwrap();
    let _daemon = start_daemon(dir.path(), None);

    stdout_of(local(dir.path()).args(["completions", "zsh"]));
    stdout_of(local(dir.path()).args(["config", "list"]));
}
