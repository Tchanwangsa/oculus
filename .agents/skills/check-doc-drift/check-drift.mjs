#!/usr/bin/env node
/**
 * Deterministic half of the `check-doc-drift` skill.
 *
 * Two independent signals, reported separately because they carry very
 * different confidence:
 *
 *   BROKEN   a page cites a path that no longer exists. The page is provably
 *            wrong — no judgement needed.
 *   CHURN    source under a path a page cites changed since the checkpoint,
 *            and the page itself did not. The page may still be correct; an
 *            agent has to read it to know.
 *
 * Usage:
 *   node check-drift.mjs [--since <ref>] [--json]
 *
 * --since defaults to the checkpoint recorded in HISTORY.md, or to the repo's
 * first commit when there is no checkpoint yet.
 */

import { execSync } from "node:child_process";
import { existsSync, readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const SKILL_DIR = dirname(fileURLToPath(import.meta.url));
const REPO = resolve(SKILL_DIR, "../../..");
const DOCS = "docs";
const HISTORY = `${SKILL_DIR}/HISTORY.md`;

const args = process.argv.slice(2);
const asJson = args.includes("--json");
const sinceIdx = args.indexOf("--since");
const sinceArg = sinceIdx === -1 ? undefined : args[sinceIdx + 1];

const git = (cmd) =>
  execSync(`git ${cmd}`, { cwd: REPO, encoding: "utf8", maxBuffer: 64 * 1024 * 1024 });

// Source trees a doc page is expected to describe. `sidecar/` was dropped when
// the Python process left the app: nothing there ships any more, so a change in
// it is not a doc obligation.
const WATCHED = /^app\//;

// All citations in docs/ are written repo-relative. A backticked span is
// treated as a path claim only if it names a real source tree and contains
// no glob, placeholder, or brace expansion.
const LOOKS_LIKE_PATH = /^(app|docs|\.agents|\.claude)\/[\w@./[\]-]*$/;

function isCheckable(s) {
  if (!LOOKS_LIKE_PATH.test(s)) return false;
  if (/[*?<>{}]/.test(s)) return false;
  // Gitignored artifacts and user data are absent by design.
  if (/(^|\/)\.env/.test(s)) return false;
  if (/(^|\/)(\.venv|node_modules|target|dist|data)(\/|$)/.test(s)) return false;
  if (s.startsWith("app/src-tauri/binaries")) return false;
  if (s.endsWith(".db")) return false;
  return true;
}

// --others --exclude-standard so brand-new, not-yet-committed pages count too.
const docFiles = [
  ...new Set(
    git(`ls-files --cached --others --exclude-standard ${DOCS}`)
      .trim()
      .split("\n")
      .filter((f) => f.endsWith(".md")),
  ),
];

// ── Signal 1: citations that no longer resolve ──────────────────────────────
const broken = [];
const citations = new Map(); // doc -> Set of cited repo-relative paths

for (const doc of docFiles) {
  const text = readFileSync(`${REPO}/${doc}`, "utf8");
  const resolved = new Set();
  const seen = new Set();

  for (const m of text.matchAll(/`([^`\n]+)`/g)) {
    const raw = m[1].trim().replace(/\/+$/, "");
    if (!isCheckable(raw) || seen.has(raw)) continue;
    seen.add(raw);

    if (existsSync(`${REPO}/${raw}`)) resolved.add(raw);
    else {
      const line = text.slice(0, m.index).split("\n").length;
      broken.push({ doc, line, cited: raw });
    }
  }
  citations.set(doc, resolved);
}

// ── Signal 2: source churn under paths the docs claim to describe ───────────
let since = sinceArg;
if (!since && existsSync(HISTORY)) {
  since = readFileSync(HISTORY, "utf8").match(/<!--\s*checkpoint:\s*(\S+)\s*-->/)?.[1];
}
if (!since || since === "none") {
  since = git("rev-list --max-parents=0 HEAD").trim().split("\n").pop();
}

let range = null;
let churn = [];
let docsTouched = [];
let commits = 0;

try {
  // Silent: an unknown checkpoint is a handled case, not an error to print.
  execSync(`git cat-file -e ${since}^{commit}`, { cwd: REPO, stdio: "ignore" });
  const head = git("rev-parse --short HEAD").trim();
  range = `${git(`rev-parse --short ${since}`).trim()}..${head}`;
  commits = Number(git(`rev-list --count ${since}..HEAD`).trim());

  const changed = git(`diff --name-status ${since}..HEAD`)
    .trim()
    .split("\n")
    .filter(Boolean)
    .map((l) => {
      const [status, ...paths] = l.split("\t");
      return { status: status[0], path: paths[paths.length - 1], from: paths[0] };
    });

  docsTouched = changed.filter((c) => c.path.startsWith(DOCS + "/")).map((c) => c.path);
  const touchedDocs = new Set(docsTouched);
  const sourceChanges = changed.filter(
    (c) => WATCHED.test(c.path) && !/(^|\/)\.[^/]+$/.test(c.path) && !/\.(json|lock)$/.test(c.path),
  );

  // A page "covers" a change when it cites the file or any ancestor directory.
  const covers = (cited, changedPath) =>
    changedPath === cited || changedPath.startsWith(cited.replace(/\/?$/, "/"));

  // Citations coarse enough to swallow a whole tree (`app/src`) match every
  // commit and tell you nothing. Require real specificity.
  const specific = new Map();
  for (const [doc, cited] of citations) {
    if (touchedDocs.has(doc)) continue; // already updated in this range
    specific.set(doc, [...cited].filter((c) => c.split("/").filter(Boolean).length >= 3));
  }

  // Attribute each changed file to exactly one page — the one whose citation
  // matches most specifically. Without this a change fans out to every page
  // with an ancestor citation, and the broadest page always looks worst.
  const byDoc = new Map();
  for (const change of sourceChanges) {
    let best = null;
    for (const [doc, cited] of specific) {
      for (const c of cited) {
        if (!covers(c, change.path) && !(change.from && covers(c, change.from))) continue;
        if (!best || c.length > best.cited.length) best = { doc, cited: c };
      }
    }
    if (!best) continue;
    if (!byDoc.has(best.doc)) byDoc.set(best.doc, { doc: best.doc, removed: [], changed: [] });
    const entry = byDoc.get(best.doc);
    const bucket = change.status === "D" || change.status === "R" ? entry.removed : entry.changed;
    if (!bucket.includes(change.path)) bucket.push(change.path);
  }
  churn = [...byDoc.values()].sort(
    (a, b) => b.removed.length - a.removed.length || b.changed.length - a.changed.length,
  );
} catch {
  range = null; // checkpoint ref is not in this clone's history
}

const report = {
  since,
  range,
  commits,
  docsTouchedInRange: docsTouched,
  broken,
  churn,
};

if (asJson) {
  console.log(JSON.stringify(report, null, 2));
  process.exit(0);
}

const rel = (d) => d.slice(DOCS.length + 1);

console.log(`checkpoint : ${since}`);
console.log(`range      : ${range ?? "(checkpoint not in history — churn skipped)"}`);
if (range) console.log(`commits    : ${commits}`);
console.log(`docs pages : ${docFiles.length}`);

console.log(`\n── BROKEN path citations (${broken.length}) ─────────────────────`);
if (!broken.length) console.log("none — every cited path exists.");
for (const b of broken) console.log(`  ${rel(b.doc)}:${b.line}  →  ${b.cited}`);

console.log(`\n── CHURN under cited paths (${churn.length} pages) ──────────────`);
if (!churn.length) console.log("none — no source moved under a path these pages cite.");
for (const c of churn) {
  const counts = [
    c.removed.length && `${c.removed.length} removed/renamed`,
    c.changed.length && `${c.changed.length} changed`,
  ]
    .filter(Boolean)
    .join(", ");
  console.log(`  ${rel(c.doc)}  (${counts})`);
  for (const p of [...c.removed, ...c.changed].slice(0, 6)) console.log(`      ${p}`);
  const extra = c.removed.length + c.changed.length - 6;
  if (extra > 0) console.log(`      … and ${extra} more`);
}
console.log();
