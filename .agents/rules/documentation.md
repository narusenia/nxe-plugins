---
paths:
  - "AGENTS.md"
  - "CLAUDE.md"
  - "README.md"
  - "docs/**/*.md"
  - "plugins/**/docs/**/*.md"
---

# Documentation rules

- Treat current implementation as authoritative when a planning document
  disagrees with the code, and fix the stale document in the same change.
- Roles do not overlap. `.agents/rules/` states what must hold,
  `docs/specifications/` and `plugins/*/docs/specifications/` state intended
  behaviour, `plugins/*/docs/requirements/` states what the product must do
  and how it is accepted, and `docs/implementation/` states what is being
  built and in what order. Do not write the same content in two of them.
- A plugin's documents live under `plugins/<name>/docs/`. Anything that spans
  plugins — crate layout, build, release, cross-plugin order — lives under
  `docs/`.
- **`docs/implementation/backlog.md` and `roadmap.md` are monorepo-wide.** A
  new implementation unit in a plugin's plan is not discoverable until it has a
  row in the backlog. Add the row in the same change.
- The backlog says *what exists and what its state is*; the roadmap says *in
  what order and why*. Do not put ordering rationale in the backlog or unit
  descriptions in the roadmap.
- A completed unit keeps its row and becomes `✅` with its PR number, or its
  commit SHA while work goes straight to `main`. Rows are not deleted.
- Do not describe a planned feature as implemented behaviour.
- Keep `AGENTS.md` short and durable; task-specific plans belong in
  `docs/implementation/` or a plugin's plan.
- Keep `CLAUDE.md` as the thin `@AGENTS.md` import so there is one canonical
  entry point.
