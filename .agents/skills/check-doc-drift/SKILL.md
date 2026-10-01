---
name: check-doc-drift
description: Audit the docs under docs/ for staleness against the code — broken path citations and links, history phrasing, and source churn since the last checked commit. Use when asked to check, audit, or reconcile the docs, before a handover, or when you suspect a page no longer matches the code.
---

# Check the docs for drift

```bash
node .agents/skills/check-doc-drift/check-drift.mjs   # --since <ref>, --json
```

- **BROKEN** — a cited path or a `./page.md#anchor` link doesn't resolve. The
  page is wrong: fix the citation, or delete the claim if the thing is gone.
  Fix all of these.
- **HISTORY** — a line narrates the past ("used to", "no longer", "was
  removed"…). Rewrite it in the present tense or delete it.
- **CHURN** — source under a cited path changed since the checkpoint and the
  page didn't. This is a reading list, not a verdict: read the page against the
  changed files and fix only what's actually wrong.

Make the edits with `write-docs`. When BROKEN and HISTORY are empty and churn
is reviewed, write the current short SHA into `CHECKPOINT` in this directory.
