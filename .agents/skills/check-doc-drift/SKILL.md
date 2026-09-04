---
name: check-doc-drift
description: Audit the docs under docs/ for staleness against the code, using broken path citations and git history since the last checked commit. Use when asked to check, audit, or reconcile the docs, before a handover, on a periodic sweep, or when you suspect a page no longer matches the code.
---

# Check the docs for drift

Answers one question: **which pages no longer match the code?**

This is the sweep. Day-to-day upkeep belongs in `write-docs`, which runs as
part of the change that caused the drift. Use this skill when nobody did
that, or to confirm nobody has to.

## Run the scan

```bash
node .agents/skills/check-doc-drift/check-drift.mjs
```

Options: `--since <ref>` overrides the checkpoint, `--json` for machine
output.

It reports two signals with deliberately different confidence.

### BROKEN — the page is provably wrong

A page cites a repo-relative path in backticks that no longer exists on disk.
No judgement required: either the path moved and the page must be updated, or
the feature is gone and the section must be deleted.

This is the highest-value output. **Fix all of it.** For each finding, locate
where the code went (`git log --diff-filter=D -- <path>`, or search for the
symbol) and correct the citation, or remove the claim if the thing is gone.

The scan only checks citations specific enough to be real claims — globs,
`<placeholders>`, and gitignored artifacts (`.venv`, `data/`, `*.db`,
`app/src-tauri/binaries/`) are skipped — so a finding here is almost never a
false positive. Still confirm before editing; a path can be typo'd rather
than stale.

### CHURN — the page may be stale

Source changed under a path the page cites, and the page itself was not
touched in the same range. Each changed file is attributed to the single page
whose citation matches most specifically, so the list is a partition, not a
fan-out.

This signal cannot tell a rename from a rewrite. Treat it as a **read
list**, ranked: start at the top, open the page, open the changed files,
decide. Rows with `removed/renamed` counts first — those usually mean a
structural change the page's `## Where` table has not caught up with.

Do not rewrite a page just because it appears here. Most churn leaves a
high-level page correct — that is the point of keeping pages high-level.

## Fix what you find

Use the `write-docs` skill for the edits. Scope discipline: correct what is
wrong, delete what is obsolete, leave the rest alone. A sweep that rewrites
every page it touches destroys the git signal that makes the next sweep
cheap.

Verify: re-run the scan; BROKEN should be empty.

## Record the run

Prepend an entry to `HISTORY.md` in this directory and move the checkpoint
comment to the commit you verified against. Format:

```md
<!-- checkpoint: <short-sha> -->

## <YYYY-MM-DD> · <model-id> · `<range>`

- **Scanned:** N commits, M pages
- **Broken:** N found, N fixed
- **Churn:** N pages flagged, N reviewed, N updated
- **Updated:** `docs/one.md`, `docs/two.md`
- **Deferred:** `docs/three.md` — <why, in a few words>
```

Rules:

- **Always advance the checkpoint** to the commit you scanned, even if you
  deferred work — the checkpoint means "everything up to here has been
  looked at", not "everything up to here is perfect".
- Record the model id you are actually running as.
- Newest entries first. If you found nothing, still write the entry —
  "checked, clean" is the most useful thing a handover can read.

`HISTORY.md` is committed: the checkpoint is shared state. Conflicts resolve
by taking the newer SHA and keeping both entries.
