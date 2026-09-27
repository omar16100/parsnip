# Publish remote mode to main

Status: **in review** (PR from `feat/remote-client-mode-v2`). Nothing published to crates.io,
no tag, no release, no Homebrew formula.

## Context

Remote client mode ([28072026_remote_mode_plan.md](28072026_remote_mode_plan.md)) was built
on a local branch and never pushed. Before publishing it:

- One unpublished commit had accidentally added a local backup file. It had to be removed
  from every outgoing commit, not just from the tip, because a later deletion does not undo
  a push.
- The README, website and `llms.txt` had install commands and claims that did not match the
  code or the published artefacts.
- Review of the race fix found paths that still bypassed it.

## Approach

1. Keep the original branch untouched; rebuild it as `feat/remote-client-mode-v2`, editing the
   offending commit in place (`git rm --cached`, `.backups/` added to `.gitignore`) and
   replaying its descendants. The deploy plist's hardcoded home directory became a
   `YOUR_USERNAME` placeholder in the same commit.
2. Verify no path under `.backups/` and no copy of that blob is reachable from the new branch;
   scan all outgoing commits (diffs and messages) for personal data and run gitleaks.
3. Fix what review found, as separate commits on top.
4. Correct the docs, then review the full diff against `main` before pushing.

## Phases

| Phase | Outcome |
|---|---|
| Scrub | Backup file gone from all 11 outgoing commits; `.backups/` ignored; plist log paths templated |
| Scrub 2 | After review, owner-environment specifics in `todo.md` and the 28072026 plan doc (shell profile file names, local backup file names, private database counts) replaced with generic wording in every outgoing commit, by a tree filter limited to this branch's range |
| CI parity | Clippy lints raised by current stable fixed (`sort_by_key`, deprecated `cargo_bin`, unreaped test child) |
| Race | MCP tool calls in the daemon and MCP proxies now resolve projects through the same atomic path as storage RPC clients (`StorageBackend::get_or_create_project`, dispatcher lock, `RemoteStorage` override). New `crates/parsnip-mcp/tests/project_race.rs` fails without the fix (1 of 6 and 1 of 8 entities reachable) |
| Hardening | Token compared without early exit; warning when the daemon runs without a token; logs to stderr so the stdio JSON-RPC stream stays clean; `--local` overrides an exported `PARSNIP_SERVER` instead of being rejected by clap; implementation notes kept out of `--help` |
| Review round 1 | Codex review (`oss_sweep_parsnip_remote_mode.txt`). Applied: `project create`/`import` through `get_or_create_project` and a dispatcher guard against rebinding a project name; pagination clamped (a zero page size aborted the daemon); loopback `Host` check for a tokenless daemon; bind host resolved and vetted before storage opens; token hidden from `--help` and refused when empty; `search/query` not broadcast over SSE; multi-project scope honoured; client redirects disabled; `--format` checked before mutations; `remote-tls` test gating and CLI feature; MSRV 1.85 (redb 2.6.3 needs it; `cargo +1.85.0 check` passes); docs corrected for hybrid, fuzzy threshold, `--quiet`, import naming, `-d` in llms.txt; plist fails closed on a missing token; test daemon retries when it loses the port race (seen as flaky "Address already in use") |
| Docs | README: `cargo install parsnip-cli` (the `parsnip` crate on crates.io is unrelated), 0.1.0 vs source install, removed nonexistent `vector` feature and search mode, performance numbers labelled as targets, real environment variables, Remote mode section, fixed MCP config (`serve`, not `mcp`), 13 tools. Site and `llms.txt`: removed the Homebrew command (no formula exists in the tap), replaced the unresolvable `parsnip.sh/install` shell command with the raw GitHub URL, same claim fixes. LICENSE-MIT year 2025; LICENSE-APACHE now carries the full license text |

## Decisions

- **No version bump beyond the existing 0.2.0 in `Cargo.toml`, no tag.** The release
  workflow runs only on `v*` tags and the Homebrew workflow only on a published release, so
  merging to `main` runs CI and nothing else.
- **Lost updates and project deletion are documented, not fixed.** Entity updates are
  read-modify-write from the client, so concurrent edits of one entity are last-write-wins,
  and deleting a project while another client writes to it can strand that write. Both
  predate remote mode (the SSE server always had concurrent clients); fixing them needs
  transactional or compare-and-swap storage RPCs. Listed in the README's known limits.
- **Commit identity.** `omar shabab <omarshabab55@gmail.com>` is the owner's chosen public
  identity for commits, so it is not treated as personal data to scrub.
- **SSE sessions.** MCP responses are still broadcast to every authenticated `/sse`
  subscriber, as in 0.1.0. Per-session routing is a follow-up.

## Verification

- `cargo fmt --all -- --check`, `cargo clippy --workspace -- -D warnings` (as CI) and
  `cargo clippy --workspace --all-targets --all-features -- -D warnings`.
- `cargo test --workspace`, including `crates/parsnip-cli/tests/remote_mode.rs`,
  `crates/parsnip-cli/tests/output_formats.rs` and the new race tests.
- `cargo install parsnip-cli` from crates.io installs `parsnip 0.1.0` (checked 27 Sep 2026).
- `cargo +1.85.0 check --workspace --locked` passes (MSRV).
- History checks: no `.backups` path in any tree reachable from the branch; gitleaks clean on
  the outgoing range and the working tree.
- After merge: CI green on `main`, `.backups` absent from `main`'s tree via the GitHub API,
  gitleaks clean on a fresh clone, Pages site returns 200, `cargo install --git` works.
