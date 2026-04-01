---
name: zo-changelog
description: >
  Creates or updates a zo changelog file. Use when user says "changelog",
  "changelogs", "log changes", "update changelog", or after a commit
  that introduces notable changes. Do NOT use for trivial changes like
  typo fixes or formatting.
---

# zo Changelog

Create or update a changelog entry in `crates/compiler/zo-notes/public/changelogs/`.

## Naming Convention

```
ZO-CL<NN>-<DD>-<MM>-<YYYY>.md
```

- `<NN>` — sequential number (zero-padded, 2 digits). check existing files and increment.
- `<DD>-<MM>-<YYYY>` — date in day-month-year format.
- example: `ZO-CL01-01-04-2026.md`, `ZO-CL02-15-04-2026.md`.

## Workflow

### Step 1: Check existing changelogs

```bash
ls crates/compiler/zo-notes/public/changelogs/
```

Determine the next sequence number and whether today's date already has a file.

### Step 2: Create or update

- **If a file for today exists**: update it — append new entries under the appropriate section.
- **If no file for today**: create a new file with the next sequence number.

### Step 3: File structure

```markdown
# ZO-CL<NN> — <DD>-<MM>-<YYYY>.

## <crate name> (<crate>).

- bullet point describing the change. lowercase. concise but specific.

## tests.

- bullet point per test file added or updated.
```

### Rules

- group entries by crate (e.g. `codegen (zo-codegen-arm)`, `parser (zo-parser)`, `SiR (zo-sir)`).
- `tests` section always last.
- each bullet starts lowercase, no trailing period.
- describe the *what* and *why*, not the *how*.
- reference function/type names in backticks.
- if a change fixes a bug, say what was broken before.
