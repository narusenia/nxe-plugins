# git rules

## Commit messages

**Conventional Commits, one line, English.** No body, no trailer, no wrapped
second paragraph.

```text
<type>: <what changed, concretely>
```

- **`<type>` is required.** `feat` / `fix` / `docs` / `refactor` / `test` /
  `perf` / `style` / `build` / `ci` / `chore`. A scope in parentheses is
  allowed but rarely earns its place in this repository — the paths in the
  diff already say where.
- **Concrete names only.** `fix: hang the last axis label off the right edge`,
  not `fix: layout issue`. The subject names the thing that changed and what
  happened to it.
- **Never process words.** No `phase1`, `step2`, `codex review`,
  `refactor pass`, `wip`, `address feedback`. These describe the session, not
  the change, and are worthless when read back from `git log` a year later.
- **Imperative or plain past both read fine**; consistency inside one series
  matters more than the mood.

## Branches

Concrete feature or fix names — `mentor-skill`, `fix-token-parser`. No phase
numbers, no step numbers, no process labels.

## Pull requests

The title follows the same rule as a commit: Conventional Commits prefix, one
line, concrete.

## Why this is written down

**The prefix drifted away silently for 22 commits** (2026-08-26 to 08-27) and
nobody noticed until the log was read back. Nothing enforces it — lefthook runs
`fmt`, `clippy` and `tests`, not a commit-message linter — so the rule has to
live somewhere it will be read.

**History is not rewritten to fix this.** The drifted commits are on `main` and
pushed; a force push to repair message formatting costs more than it buys.
