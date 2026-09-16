---
name: write-docs
description: Update the Oculus docs under docs/ after changing code. Use whenever you add, move, rename, or delete a feature, module, page, store, endpoint, or workflow in app/, when a doc contradicts the code, or when asked to document how something works.
---

# Keep the docs true

Docs live in `docs/` as plain Markdown and ship in the **same change** as the
code they describe. A commit that moves a feature and leaves its page stale
is incomplete. Nothing enforces this automatically — it is on you.

Read `read-docs` first if you have not oriented yet.

## What belongs here

These pages answer **"where does this live and how does it connect?"** — a
new engineer's map, plus the measured facts that justify non-obvious
decisions.

Write:
- Where a feature's UI, Rust commands, and schema live
- How the processes connect, and which module owns which responsibility
- Non-obvious constraints, gotchas, and why a shape is the way it is —
  especially anything *measured* (benchmarks, timings) that a refactor could
  silently undo

Do not write:
- Line-by-line walkthroughs, function signatures, or copies of the code
- Anything that changes every time someone touches a file
- Conventions — those belong in the root `CLAUDE.md`

If a fact is already obvious from reading one file, leave it out.

## Editing a page

1. Find the page via the map in `docs/index.md`.
2. Most pages carry a `## Where` table mapping pieces to paths. Keep those
   paths exact, current, and **repo-relative** (`app/src-tauri/src/sync.rs`,
   not `src/sync.rs`) — `check-doc-drift` resolves every backticked path
   against the filesystem, so citations are load-bearing.
3. Every page needs a `## How it connects` section with facts that could not
   be guessed from the file tree. A page that is only a `Where` table tells
   the reader nothing `ls` wouldn't.
4. Delete claims the change made false. Removing a stale paragraph is as
   valuable as adding a new one.

## Adding a page

Create `docs/<topic>.md`:

```md
# Topic name

What it is, in a line or two.

## Where

| Piece | Location |
| --- | --- |
| Thing | `app/src/...` |

## How it connects

- At least one fact you had to open a file to learn.
```

Then add a row to the table in `docs/index.md`, and — if agents should be
routed there for a class of task — a row to the doc maps in `CLAUDE.md` and
`.agents/skills/read-docs/SKILL.md`.

### The "How it connects" rule

`Where` says what exists. `How it connects` says what a reader would get
wrong. Good bullets look like:

- *"In the app the frontend writes the scrape tables; the CLI writes the same
  rows via `store.rs` — change a table and both writers move together."*
- *"`.pages.json` is only written when a parse finishes, so an interrupted
  parse never reads as done."*
- *"Averaging image and text vectors scored worse than image alone — don't
  hybridize at the vector level."*

Reach for: which layer owns a decision, ordering that must not change, a
constant two processes depend on, a pin or guard that looks removable and
isn't, something named misleadingly, and cross-links to the page on the
other side of the boundary.

## Style

- Plain GitHub Markdown only — no MDX, no frontmatter, no components.
- Links between docs are relative with the `.md` extension
  (`./retrieval.md`).
- Paths and identifiers in backticks; keep paths specific enough to be real
  claims (the drift checker skips globs and `<placeholders>`).

## Verify

There is no build. Verify by running the drift scan — the page you just
edited must not report BROKEN citations:

```bash
node .agents/skills/check-doc-drift/check-drift.mjs
```
