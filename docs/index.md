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

## Documents

| Document | Category | Summary |
|---|---|---|
| [c4model.md](c4model.md) | Architecture | Source of truth for containers, components and data flows. Read before architecture changes; update as part of them. |
| [28072026_remote_mode_plan.md](28072026_remote_mode_plan.md) | Plan | Remote client mode: one daemon owns the database, everything else reaches it over HTTP. |
| [spec.md](spec.md) | Reference | Original product specification. |
| [llms.txt](llms.txt) | Reference | Machine-readable project summary. |
