import { expect, test } from "bun:test";
import { readdirSync, readFileSync } from "node:fs";
import { join, relative, sep } from "node:path";

// Every path shape in the library is spelled in one Rust module,
// `app/src-tauri/src/library/layout/`, and one TypeScript file,
// `app/src/lib/files/libraryLayout.ts` (docs/architecture.md, "One module owns
// every path shape in the library").
// Anywhere else, a library folder name used as a path segment is a second
// copy that a new layout would leave behind. Tests, fixtures and comments may
// spell paths; app code asks the layout.

const APP = join(import.meta.dir, "../..");
const ROOTS = ["src", "src-tauri/src"];

/** The layout's own files. */
const OWNERS = ["src-tauri/src/library/layout/", "src/lib/files/libraryLayout.ts"];

/** Folder names the library uses: the data dir's, a subject's, and its
 *  synced kinds'. */
const CURRENT = [
  "courses",
  "Course",
  ".oculus",
  "Files",
  "Media",
  "Recordings",
  "Lessons",
  "Discussion",
  "Pages",
  "Assignments",
  "Quizzes",
  "Announcements",
  "Modules",
  "Images",
];

/** The layout before the Drive tree; only its `LEGACY` shape
 *  (`src-tauri/src/library/layout/shape.rs`) may still spell these. */
const LEGACY = [
  "lectures",
  "uploads",
  "documents",
  "canvases",
  "notebooks",
  "files",
  "lessons",
  "ed",
  "pages",
  "assignments",
  "quizzes",
  "announcements",
  "modules",
  "images",
];

const FOLDERS = [...CURRENT, ...LEGACY].map((f) => f.replace(".", "\\.")).join("|");

/** Lines that name a folder but not a library path, each with its reason. */
const ALLOWED: [file: string, text: string, why: string][] = [
  // Canvas's own URLs (`/courses/:id/files/:id`, `/pages/:slug`), parsed to find the library file they mean.
  ["src/lib/files/libraryLinks.ts", "href.includes(\"/courses/\")", "a Canvas URL"],
  ["src/lib/files/libraryLinks.ts", "/\\/files\\/(\\d+)/", "a Canvas URL"],
  ["src/lib/files/libraryLinks.ts", "/\\/pages\\/([^/?#]+)/", "a Canvas URL"],
  ["src/lib/files/libraryLinks.ts", "`/pages/${slug}`", "a Canvas URL"],
  ["src-tauri/src/pages/md/mod.rs", "after_marker(href, \"/pages/\")", "a Canvas URL"],
  ["src-tauri/src/pages/md/mod.rs", "href.contains(\"/courses/\")", "a Canvas URL"],
  ["src-tauri/src/pages/md/mod.rs", "after_marker(href, \"/files/\")", "a Canvas URL"],
  // Agent-facing examples of a citation, prose like the templates.
  ["src/components/markdown/openui/prompt.ts", "PageRef(\"courses/", "an example for agents"],
  ["src/components/markdown/openui/components/PageRef.tsx", "PageRef(\\\"courses/", "an example for agents"],
  // Canvas's course URL in a downloaded page's address.
  ["src/lib/browser/downloads/subjectGuess.ts", "/\\/courses\\/(\\d+)", "a Canvas URL"],
  // An assignment's Canvas link, matched to a deadline row.
  ["src/hooks/data/overview/comingUp.ts", "/\\/assignments\\/(\\d+)(?:[?#]|$)/", "a Canvas URL"],
  // MinerU's result archive keeps its pictures in `images/`, before the parse moves them.
  ["src-tauri/src/parse/mineru/client/extract.rs", ".join(\"images\")", "MinerU's archive"],
  ["src-tauri/src/parse/mineru/local/mod.rs", ".join(\"images\")", "MinerU's archive"],
  ["src-tauri/src/parse/mineru/client/run_batch.rs", ".join(\"images\")", "MinerU's archive"],
  ["src-tauri/src/parse/mineru/client/run_batch.rs", "format!(\"images/{target}\")", "MinerU's archive"],
  // App routes (`/subjects/:id/files/uploads`), not library paths. Stage 6 of the Drive plan rewrites these screens.
  ["src/components/tabs/tabInfo.tsx", "/^\\/subjects\\/\\d+\\/files\\/(drive|documents", "stage 6: an app route"],
  ["src/routes.tsx", "\"../files/", "stage 6: an app route"],
  ["src/components/subjects/SubjectCrumbs.tsx", "to: \"files/", "stage 6: an app route"],
];

/** A literal that is a library path, or the start of one. */
const PATH_START = new RegExp(`^(\\.\\./)*(${FOLDERS})/`);
/** A folder between two segments of a relative path. */
const PATH_MIDDLE = new RegExp(`/(${FOLDERS})/`);
const SYNCED_FILE = /^(home|syllabus)\.md$/;
/** A bare segment to look for inside a path: `"/documents/"`. */
const SEGMENT = new RegExp(`^/(${FOLDERS})/$`);
/** `join("files")` and its kin. */
const JOIN = new RegExp(`join\\(\\s*["'\`](${FOLDERS})["'\`]\\s*\\)`);
/** A regex literal anchored on a folder: `/^courses\/`, `/\/documents\//`. */
const REGEX = new RegExp(`(\\^|\\\\/)(${FOLDERS})\\\\/`);

function* sources(dir: string): Generator<string> {
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const path = join(dir, entry.name);
    if (entry.isDirectory()) {
      // Test scaffolding: `testkit/` is compiled only under `#[cfg(test)]`.
      if (["tests", "fixtures", "testkit"].includes(entry.name)) continue;
      yield* sources(path);
    } else if (/\.(rs|ts|tsx)$/.test(entry.name) && !/(^tests|_tests?|\.test)\.(rs|ts|tsx)$/.test(entry.name)) {
      yield path;
    }
  }
}

/** Code lines: comments dropped, and a Rust file's inline test module
 *  (`#[cfg(test)]` or `#[cfg(all(test, …))]` over `mod tests {`, which ends
 *  the file) left out. */
function codeLines(path: string): [number, string][] {
  const lines = readFileSync(path, "utf8").split("\n");
  const end = path.endsWith(".rs")
    ? lines.findIndex((l, i) => /^\s*#\[cfg\((all\()?test\b/.test(l) && /^\s*mod \w+\s*\{/.test(lines[i + 1] ?? ""))
    : -1;
  return lines
    .slice(0, end === -1 ? lines.length : end)
    .map((line, i): [number, string] => [i + 1, line])
    .filter(([, line]) => !/^\s*(\/\/|\/?\*)/.test(line));
}

/** String literals on a line: Rust has only `"…"`; TypeScript also `'…'` and `` `…` ``. */
function literals(line: string, rust: boolean): string[] {
  const pattern = rust ? /"((?:[^"\\]|\\.)*)"/g : /"((?:[^"\\]|\\.)*)"|'((?:[^'\\]|\\.)*)'|`((?:[^`\\]|\\.)*)`/g;
  return [...line.matchAll(pattern)].map((m) => m[1] ?? m[2] ?? m[3] ?? "");
}

/** A module specifier, which names source folders, not library ones. */
const IMPORT = /\bfrom\s*["']|\bimport\s*\(|^\s*import\s+["']/;

function names(line: string, rust: boolean): string | null {
  if (!rust && IMPORT.test(line)) return null;
  if (JOIN.test(line)) return "joins a library folder";
  if (!rust && REGEX.test(line)) return "matches a library folder";
  for (const lit of literals(line, rust)) {
    if (PATH_START.test(lit) || SYNCED_FILE.test(lit) || SEGMENT.test(lit)) return `spells ${JSON.stringify(lit)}`;
    // A URL path (`/courses/…`, `${base}/courses/…`) is someone else's layout.
    const url = /^[/{$]/.test(lit) || lit.includes("://");
    if (!url && PATH_MIDDLE.test(lit)) return `spells ${JSON.stringify(lit)}`;
  }
  return null;
}

test("only the layout module spells the library's folders", () => {
  const hits: string[] = [];
  for (const root of ROOTS) {
    for (const path of sources(join(APP, root))) {
      const rel = relative(APP, path).split(sep).join("/");
      if (OWNERS.some((owner) => rel.startsWith(owner))) continue;
      const rust = rel.endsWith(".rs");
      for (const [n, line] of codeLines(path)) {
        if (ALLOWED.some(([file, text]) => file === rel && line.includes(text))) continue;
        const what = names(line, rust);
        if (what) hits.push(`${rel}:${n} ${what}: ${line.trim()}`);
      }
    }
  }
  expect(hits).toEqual([]);
});

test("every allowance still matches a line", () => {
  const stale = ALLOWED.filter(
    ([file, text]) => !readFileSync(join(APP, file), "utf8").split("\n").some((l) => l.includes(text)),
  ).map(([file, text]) => `${file}: ${text}`);
  expect(stale).toEqual([]);
});

test("the old folder names are spelled only in the LEGACY shape", () => {
  // Category names (`"lessons"`, `"ed"`) share some of these spellings, so
  // only the shape tables, which hold folder names alone, are checked.
  const legacy = new RegExp(`"(${LEGACY.join("|")})"`);
  const path = join(APP, OWNERS[0], "shape.rs");
  const lines = readFileSync(path, "utf8").split("\n");
  const start = lines.findIndex((l) => l.includes("const LEGACY: Shape"));
  const end = lines.findIndex((l, i) => i > start && l === "};");
  expect(start).toBeGreaterThan(-1);
  const hits = lines
    .map((line, i): [number, string] => [i + 1, line])
    .filter(([n, line]) => (n - 1 < start || n - 1 > end) && legacy.test(line))
    .map(([n, line]) => `shape.rs:${n} ${line.trim()}`);
  expect(hits).toEqual([]);
});
