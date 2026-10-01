---
name: write-docs
description: Update the Oculus docs under docs/ after changing code. Use whenever you add, move, rename, replace or delete a feature, module, page, store, endpoint or workflow in app/, when a doc contradicts the code, or when asked to document how something works.
---

# Keep the docs true

Docs describe the code **as it is now**, and ship in the same commit as the
code they describe. They are a map for understanding the codebase, not an audit
trail.

## Removing is half the job

When a feature is removed, replaced, renamed or changes shape, find every
mention and fix or delete it — not only the obvious page:

```bash
git grep -n -i "<name>\|<old path>\|<old identifier>" -- docs app/src app/src-tauri/src
```

- Delete the mention outright. Don't leave "X was removed", "no longer",
  "used to", "replaced by", or a note that it might come back.
- The only record of what was removed is the short "Don't re-add" list in the
  root `CLAUDE.md`, and only when agents keep trying to re-add it.
- Code comments that cite a doc section move with it.

## Page shape

```md
# Topic

One or two lines: what it is.

## Where

| Piece | Location |
| --- | --- |
| Thing | `app/src/...` |

## <A heading that states a claim>

Short paragraphs, bullets for unordered sets, numbered lists for flows.

## Gotchas

- One line each: the rule, and what breaks if you ignore it.
```

- **Headings are claims** ("Parses are never retried on the other engine"),
  not categories. No "How it connects", "Notes" or "Details".
- **Gotchas are one-liners** at the bottom. If a gotcha needs a paragraph, the
  paragraph belongs in the section it concerns and the gotcha points there.
- **Aim for 100–250 lines per page.** Past that, cut before you split.

## What to write

Write what a reader would get wrong from the code alone: which module owns a
decision, ordering that must not change, a guard that looks removable and
isn't, a measured fact behind a design choice (as one line — the number and
the choice it justifies).

Don't write:
- History: how it got here, incidents, earlier versions, dates, "measured once
  at…". That goes in the commit message.
- Anything obvious from reading the one file it describes.
- Walkthroughs, signatures, copies of code, or the same fact on two pages —
  state it once and link.

## Citations

- Paths in backticks, repo-relative (`app/src-tauri/src/sync.rs`) —
  `check-doc-drift` resolves every one.
- Links between pages are relative with `.md` (`./retrieval.md#anchor`).
- Add a new page to the table in `docs/index.md`.

## Verify

```bash
node .agents/skills/check-doc-drift/check-drift.mjs
```

It must report no broken paths or links on the pages you touched, and no
history phrasing.
