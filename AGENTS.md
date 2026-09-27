# Repository Guidelines

## Project Structure & Module Organization

- `Cargo.toml` defines a Rust workspace.
- `crates/` contains workspace members:
  - `crates/parsnip-core/`: domain types + the `KnowledgeGraph` trait.
  - `crates/parsnip-storage/`: storage backends + the `StorageBackend` trait.
    - `crates/parsnip-storage/src/remote/`: the storage RPC. `protocol` is the wire format,
      `dispatch` is the server side, `client` is `RemoteStorage` (behind the `remote` feature).
  - `crates/parsnip-{search,cli,mcp}/`: search engines, the CLI, and the MCP server.
- `docs/` contains design docs. Start with `docs/index.md`; `docs/c4model.md` is the
  architecture source of truth and must be updated with any architecture change.
- `deploy/` holds deployment artefacts (the launchd plist for the daemon).
- Integration tests live in `crates/<pkg>/tests/`. A workspace-root `tests/` directory is
  not compiled by cargo.
- `benches/` is reserved for benchmarks.
- `assets/` is reserved for non-code assets.

## Build, Test, and Development Commands

- `cargo fmt` — format the codebase (rustfmt defaults).
- `cargo clippy --workspace --all-targets --all-features` — lint for common Rust issues.
- `cargo test --workspace --all-features` — run unit + integration tests.
- Default features include `sse` (the HTTP/SSE server) and `remote` (the HTTP client).
  Keep a `--no-default-features` build working so the feature-gated fallbacks do not rot.
- `cargo doc --workspace --no-deps` — build API docs locally.

Note: if `cargo` fails due to missing workspace members, add the missing crate `Cargo.toml` files or temporarily remove unfinished members from `[workspace].members`.

## Coding Style & Naming Conventions

- Rust edition is 2021; keep the MSRV in `Cargo.toml` (`rust-version = "1.85"`, required by the locked redb).
- Prefer workspace dependencies: add to `[workspace.dependencies]`, then reference via `{ workspace = true }`.
- Naming: `snake_case` for modules/functions, `UpperCamelCase` for types/traits, `SCREAMING_SNAKE_CASE` for constants.
- Errors: use `thiserror` for library error enums; use `anyhow` for binary entrypoints.

## Testing Guidelines

- Unit tests: colocate via `#[cfg(test)] mod tests` next to the code under test.
- Integration tests: add focused scenarios under `tests/*.rs` that exercise public APIs.
- Keep tests deterministic; use `tempfile` for filesystem state.

## Commit & Pull Request Guidelines

- Git history is not available in this workspace; use Conventional Commits where possible:
  - `feat(core): ...`, `fix(storage): ...`, `chore: ...`
- PRs should include: a clear description + rationale, validation steps (e.g. `cargo test`, `cargo clippy`), linked issues, and notes for any breaking changes.

## Agent-Specific Instructions

- Keep patches small and focused; avoid repo-wide refactors/reformatting.
- Don’t add new dependencies without a concrete need and a short justification in the PR.
