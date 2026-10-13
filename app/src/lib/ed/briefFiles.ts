import { resolveBeside } from "./lessons";

/** A Files list entry, `- [name](href)`, with `name` as Rust escapes it. */
const LIST_ENTRY = /^- \[((?:\\.|[^\]\\])*)\]\([^)]*\)$/;
const FENCE_OPEN = /^(`{3,})[^`]*$/;

/**
 * A challenge slide's `.md` without the inline copy of each workspace file.
 * The sync ends the brief with `**Files**`, a list of links, then each small
 * text file as a `**<name>**` paragraph over a fenced copy, `<name>` being
 * the list's link text (`files_md` in `sources/ed/lesson_md/body.rs`). The
 * list stays; every copy after it whose name the list holds goes. Anything
 * else ends the cut, so text that is not those copies is never dropped.
 */
export function dropInlineFiles(markdown: string): string {
  const lines = markdown.split("\n");
  let start = -1;
  for (let i = lines.length - 3; i >= 0; i--) {
    if (lines[i] === "**Files**" && lines[i + 1] === "" && LIST_ENTRY.test(lines[i + 2])) {
      start = i;
      break;
    }
  }
  if (start < 0) return markdown;
  const names = new Set<string>();
  let at = start + 2;
  for (let m; at < lines.length && (m = LIST_ENTRY.exec(lines[at])); at++) names.add(m[1]);

  const keep = lines.slice(0, at);
  let cut = at;
  for (;;) {
    let i = cut;
    while (i < lines.length && lines[i] === "") i++;
    const name = /^\*\*(.+)\*\*$/.exec(lines[i] ?? "")?.[1];
    if (name == null || !names.has(name) || lines[i + 1] !== "") break;
    const fence = FENCE_OPEN.exec(lines[i + 2] ?? "")?.[1];
    if (!fence) break;
    const close = lines.indexOf(fence, i + 3);
    if (close < 0) break;
    cut = close + 1;
  }
  return [...keep, ...lines.slice(cut)].join("\n");
}

/**
 * The workspace path a link in slide `briefPath`'s `.md` points at, or null
 * when it leads elsewhere. A challenge's files sit in the folder named for
 * the slide's stem, so `./03-q1a/src/a.py` from `…/03-q1a.md` is `src/a.py`.
 */
export function workspaceFilePath(href: string, briefPath: string): string | null {
  if (!href.startsWith("./")) return null;
  const segments = href.slice(2).split("/");
  let decoded: string;
  try {
    decoded = segments.map(decodeURIComponent).join("/");
  } catch {
    return null;
  }
  const stem = briefPath.replace(/\.md$/, "");
  const path = resolveBeside(briefPath, decoded);
  return path.startsWith(`${stem}/`) ? path.slice(stem.length + 1) : null;
}
