# Security dependency updates

Status: **merged** (#8, 27 Sep 2026). Lockfile and MSRV only; no source changes.

## Context

Dependabot alerts were enabled on 27 Sep 2026 and opened 7 alerts against `Cargo.lock`,
all in transitive dependencies. Dependabot also opened one PR per crate (#1 time, #3
lz4_flex, #4 bytes, #5 oneshot, #7 rand). Those PRs predate the remote mode merge (#6), and
#5 failed CI on an unrelated clippy lint that #6 already fixed on `main`.

| Alert | Crate | Advisory | Pulled in by | Fix |
|---|---|---|---|---|
| 5 (high) | lz4_flex 0.11.5 | GHSA-vvp9-7p8x-rfvv | tantivy | 0.11.6 |
| 2 (high) | oneshot 0.1.11 | GHSA-rvr2-r3pv-5m4p | tantivy | 0.1.13 |
| 4 (medium) | time 0.3.44 | GHSA-r6v5-fh4h-64xc | tantivy, tantivy-common | 0.3.55 |
| 3 (medium) | bytes 1.11.0 | GHSA-434x-w66g-qw3r | axum, http, hyper, reqwest, tokio, tower-http | 1.12.1 |
| 6, 7 (low) | rand 0.9.2, 0.8.5 | GHSA-cq8v-f236-94qc | ulid (0.9), rand_distr via tantivy-stacker (0.8) | 0.9.5, 0.8.8 |
| 1 (low) | lru 0.12.5 | GHSA-rhfx-m35p-ff5j | tantivy 0.22 (`lru = "^0.12"`) | none in range, see below |

## Approach

1. One consolidated PR from `main`:
   `cargo update -p rand@0.8.5 -p rand@0.9.2 -p lz4_flex -p time -p bytes -p oneshot`.
   Cargo also moved `deranged`, `num-conv`, `time-core` and `time-macros` along with `time`.
2. MSRV 1.85 to 1.88. Every patched `time` release (0.3.47 onwards) declares
   `rust-version = 1.88`, so the declared MSRV had to follow the lockfile. The README badge
   and AGENTS.md follow.
3. Close the five Dependabot PRs as superseded.
4. `lru`: the fix (0.16.3) is outside tantivy 0.22's `^0.12` requirement. Only tantivy
   0.26 moves to `lru ^0.16.3`, which is an index-format and API upgrade, not a lockfile
   change. The advisory is a Stacked Borrows violation in `IterMut`. tantivy 0.22.1 keeps
   its `LruCache` private inside the doc store's block cache and only calls `new`, `get`,
   `put` and `len` (plus `peek_lru` in its own tests); parsnip does not depend on `lru`
   directly. The alert is
   dismissed as tolerable risk with that reason, and a tantivy upgrade is listed as a
   follow-up in the root `todo.md`.

## Verification

- `cargo test --workspace --locked`: 114 passed.
- `cargo test --workspace --all-features --locked`: 121 passed.
- `cargo clippy --workspace --all-targets --locked -- -D warnings` (Rust 1.95.0 and 1.98.0) and
  `cargo fmt --check`: clean.
- Codex review (`oss_sweep_parsnip_security_deps.txt`): no blocker or major findings. Applied:
  tantivy calls `peek_lru` only in its tests; clippy rerun on the CI toolchain line (1.98).
- `cargo +1.88.0 check --workspace --all-targets --locked`: passes.
- `cargo build -p parsnip-cli --no-default-features --features redb` and `--features sqlite`:
  pass. (`--no-default-features` with no backend at all does not compile; that was already
  true on `main`.)
- CLI smoke on a temporary data dir: `entity add`, then `search --mode fulltext` and
  `--mode hybrid` return the entity through the tantivy index.
- CI on the PR, then on `main` after merge; open alerts re-queried after merge.
