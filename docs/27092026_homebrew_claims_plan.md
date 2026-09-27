# Homebrew and install claims on the published docs

Status: **merged** (#9, 27 Sep 2026). Docs only; no code changes.

## Context

GitHub Pages publishes everything under `docs/`, so `docs/todo.md` and `docs/spec.md` are
served as `todo.html` and `spec.html`. Both gave a Homebrew install command (the
`omar16100/tap/parsnip` formula in todo, a `parsnip` formula in spec). Neither works:
`omar16100/homebrew-tap` has only a `batteryconsole` formula (checked 27 Sep 2026).
The spec also listed `cargo install parsnip`, which installs an unrelated crate of that
name, plus a `ghcr.io/parsnip-ai/parsnip` image and a `github:parsnip-ai/parsnip` Nix
flake, neither of which exists.

## Approach

- `docs/todo.md`: the Distribution entry now says the Homebrew tap is planned, not
  published, and that the site's Homebrew tab was removed. The Installation block lists
  the shell installer (0.1.0 release binary), `cargo install parsnip-cli` (0.1.0) and the
  `cargo install --git` source install, matching `llms.txt` and the site.
- `docs/spec.md`: Appendix K separates what is available today from what is planned, and
  the build target table is labelled as a target.
- Swept every file under `docs/` for `brew`, `homebrew`, `cargo install`, `curl`,
  `docker`, `ghcr`, `nix` and `parsnip.sh`. `index.html` and `llms.txt` were already
  correct after the 27 Sep 2026 publish pass.

## Verification

- A search of `docs/` for a Homebrew install command returns nothing.
- After merge: the Pages build succeeds and `todo.html` no longer contains the
  Homebrew command.

## Status

- [x] Edits, review, merge (#9)
- Pages rebuild and the live `todo.html` check run after merge; the result is posted on #9.
