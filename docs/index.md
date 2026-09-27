# Documentation index

## Conventions

- Dated documents: `DDMMYYYY_topic.md` (e.g. `28072026_remote_mode_plan.md`). Use them for
  work that is tied to a point in time: plans, migrations, incident notes.
- Evergreen documents: `topic.md`. Use them for things that describe the system as it is
  now, and keep them current rather than appending history.
- Every new document gets a row in the table below.

## Categories

| Category | What belongs here | Required sections |
|---|---|---|
| Architecture | How the system is put together | Context, containers, components, data flows |
| Plan | A specific piece of work | Context, approach, phases, verification, status |
| Reference | Facts a reader needs to look up | Whatever the subject needs, kept scannable |
| Website | Published pages | Claims must match the README and the code |

## Documents

| Document | Category | Summary |
|---|---|---|
| [c4model.md](c4model.md) | Architecture | Source of truth for containers, components and data flows. Read before architecture changes; update as part of them. |
| [28072026_remote_mode_plan.md](28072026_remote_mode_plan.md) | Plan | Remote client mode: one daemon owns the database, everything else reaches it over HTTP. |
| [27092026_publish_remote_mode_plan.md](27092026_publish_remote_mode_plan.md) | Plan | Publishing remote mode to main: history scrub, fixes from review, README and site corrections. |
| [27092026_security_deps_plan.md](27092026_security_deps_plan.md) | Plan | Dependabot security alerts: lockfile updates, MSRV 1.88, and the `lru` alert that needs a tantivy upgrade. |
| [spec.md](spec.md) | Reference | Original product specification. Performance numbers in it are targets. |
| [todo.md](todo.md) | Reference | Original phase checklist from the initial build. Current work is tracked in the root [todo.md](../todo.md). |
| [llms.txt](llms.txt) | Reference | Machine-readable project summary, served by the website. |
| [index.html](index.html) | Website | GitHub Pages site (published from `docs/` on `main`). |
