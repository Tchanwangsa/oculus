import { splitLibraryPaths } from "@/lib/files/libraryPaths";
import { messageAt, parseToolMeta } from "@/lib/harness/meta";
import { shellCommands } from "@/lib/harness/oculusCli";
import type { HarnessItem } from "@/lib/harness/types";
import { mergePages, parsePages, type PageRange } from "./ranges";
import {
  isMadePath,
  isReadPath,
  libraryKey,
  studioHref,
  toLibraryPath,
  type InfoTarget,
} from "./targets";
import { recordingRel } from "@/lib/files/libraryLayout";

/**
 * Thread info's Read and Made groups, projected from a thread's rows with no
 * query: what the agent read (Read tool rows, `oculus read`, a shell reader
 * such as `cat`), what the student @-mentioned or attached, a lecture
 * thread's lecture and the moments asked at, and what the agent wrote
 * (Write and Edit rows, files it sent to `outputs/`, `oculus` commands that
 * write something a row can open). One row per file, most recent first.
 */

export interface InfoEntry {
  /** One per file: a parsed `.md` is its PDF, a lecture's files the lecture. */
  key: string;
  target: InfoTarget;
  /** What names the row's glyph and label: a library path, or a Studio
   *  file's absolute path. */
  path: string;
  /** A row named by what it did rather than a file ("Rendered the video"). */
  label?: string;
  pages: PageRange[];
  /** Lecture moments, in seconds, ascending. */
  times: number[];
  /** Made only: the first touch made the file (`new`) or changed it. */
  made?: "new" | "edited";
  /** The row the entry was last touched in; sorts the list. */
  last: number;
  /** A lecture thread's own lecture leads Read. */
  pinned?: boolean;
}

export interface ThreadInfoLists {
  read: InfoEntry[];
  made: InfoEntry[];
}

/** Commands whose path arguments are files being read. */
const READERS = new Set(["cat", "head", "tail", "sed", "nl", "less", "more", "bat", "wc"]);

/** Path arguments each CLI's file tools spell (`classify.rs`). */
const PATH_KEYS = ["file_path", "path", "notebook_path", "Path", "AbsolutePath", "TargetFile"];

/** A path under the agent's `outputs/`, as a shell line can spell it. */
const OUTPUT_PATH = /(?:^|[\s'"=(>])((?:\/[^\s'"|;&<>()]*?\/agents\/|\.\.\/agents\/|\.\/|agents\/)?outputs\/[^\s'"|;&<>()]+\.[A-Za-z0-9]+)/g;

const str = (v: unknown) => (typeof v === "string" && v.trim() ? v : null);

function toolPaths(input: unknown): string[] {
  if (!input || typeof input !== "object") return [];
  const o = input as Record<string, unknown>;
  const out = PATH_KEYS.map((k) => str(o[k])).filter((p): p is string => !!p);
  // Codex's `fileChange` names every file it touched.
  if (Array.isArray(o.paths)) out.push(...o.paths.filter((p): p is string => !!str(p)));
  // opencode's `apply_patch` names its files in the envelope's headers.
  const patch = str(o.patchText);
  if (patch) {
    for (const m of patch.matchAll(/^\*\*\* (?:Add File|Update File|Move to):\s*(.+)$/gm)) out.push(m[1].trim());
  }
  return [...new Set(out)];
}

class Lists {
  private read = new Map<string, InfoEntry>();
  private made = new Map<string, InfoEntry>();

  addRead(path: string, at: number, extra: { pages?: PageRange[]; time?: number | null; pinned?: boolean } = {}) {
    if (!isReadPath(path)) return;
    const key = libraryKey(path);
    const e = this.read.get(key) ?? { key, target: { kind: "library", path: key }, path: key, pages: [], times: [], last: at };
    e.last = Math.max(e.last, at);
    if (extra.pages?.length) e.pages = mergePages([...e.pages, ...extra.pages]);
    if (extra.time != null && !e.times.includes(extra.time)) e.times = [...e.times, extra.time].sort((a, b) => a - b);
    if (extra.pinned) e.pinned = true;
    this.read.set(key, e);
  }

  /** `raw` as the tool spelled it: a library path, or a Studio file. */
  addMade(raw: string, at: number, op: "new" | "edited") {
    const lib = toLibraryPath(raw);
    if (lib && isMadePath(lib)) return this.put(lib, { kind: "library", path: lib }, lib, at, op);
    const href = studioHref(raw);
    if (href) this.put(raw, { kind: "route", href }, raw, at, op);
  }

  addMadeTarget(key: string, target: InfoTarget, path: string, at: number, label?: string) {
    this.put(key, target, path, at, "edited", label);
  }

  private put(key: string, target: InfoTarget, path: string, at: number, op: "new" | "edited", label?: string) {
    const e = this.made.get(key);
    if (e) e.last = Math.max(e.last, at);
    else this.made.set(key, { key, target, path, pages: [], times: [], made: op, last: at, ...(label ? { label } : {}) });
  }

  done(): ThreadInfoLists {
    const byRecent = (a: InfoEntry, b: InfoEntry) =>
      Number(!!b.pinned) - Number(!!a.pinned) || b.last - a.last;
    const made = [...this.made.values()].sort(byRecent);
    const madeKeys = new Set(made.map((e) => e.key));
    // An agent reads a file before it edits it; the file is something made.
    const read = [...this.read.values()]
      .filter((e) => !madeKeys.has(e.key))
      .map((e) => {
        const page = e.pages[0]?.from;
        const at = e.times[e.times.length - 1];
        return { ...e, target: { kind: "library" as const, path: e.key, ...(page ? { page } : {}), ...(at != null ? { at } : {}) } };
      })
      .sort(byRecent);
    return { read, made };
  }
}

/** One shell line's reads, outputs and `oculus` writes. */
function scanCommand(line: string, at: number, lists: Lists) {
  const commands = shellCommands(line);
  const readArgs = new Set<string>();
  for (const words of commands) {
    const name = words[0]?.split("/").pop() ?? "";
    if (READERS.has(name)) {
      for (const w of words.slice(1)) {
        readArgs.add(w);
        const lib = toLibraryPath(w);
        if (lib) lists.addRead(lib, at);
      }
      continue;
    }
    const oculus = words.findIndex((w, i) => i < 3 && /^oculus(\.exe)?$/i.test(w.split(/[/\\]/).pop() ?? ""));
    if (oculus >= 0) scanOculus(words.slice(oculus + 1), at, lists);
  }
  for (const m of line.matchAll(OUTPUT_PATH)) {
    if (!readArgs.has(m[1])) lists.addMade(m[1], at, "new");
  }
}

const flagValue = (words: string[], ...names: string[]) => {
  for (let i = 0; i < words.length - 1; i++) {
    if (names.includes(words[i])) return words[i + 1];
    const eq = names.map((n) => `${n}=`).find((p) => words[i].startsWith(p));
    if (eq) return words[i].slice(eq.length);
  }
  return undefined;
};

/** `oculus read`, and the writes whose result a row can open: a memory, and
 *  a Studio video's narration, frames and render. */
function scanOculus(rest: string[], at: number, lists: Lists) {
  const [cmd, action, ...more] = rest;
  if (cmd === "read") {
    const file = rest.slice(1).find((w, i, all) => !w.startsWith("-") && !["-p", "--pages", "-s", "--subject"].includes(all[i - 1]));
    const lib = toLibraryPath(file);
    if (lib) lists.addRead(lib, at, { pages: parsePages(flagValue(rest, "-p", "--pages")) });
    return;
  }
  if (cmd === "memory" && (action === "write" || action === "append" || action === "fold")) {
    const name = action === "fold" ? flagValue(more, "--into") : more[0] && !more[0].startsWith("-") ? more[0] : undefined;
    const code = flagValue(more, "-s", "--subject");
    const bucket = `agents/memories/${code ? `${code}/` : ""}`;
    const path = name ? `${bucket}${name}.md` : bucket;
    lists.addMadeTarget(path, { kind: "library", path }, path, at);
    return;
  }
  if (cmd === "studio" && (action === "render" || action === "frame" || action === "narrate")) {
    const dir = more.find((w) => !w.startsWith("-") && w.includes("/"));
    const href = dir ? studioHref(dir) : null;
    if (!href) return;
    const label = action === "render" ? "Rendered the video" : action === "narrate" ? "Narrated the video" : "Rendered a frame";
    lists.addMadeTarget(`${href}#${action}`, { kind: "route", href }, dir!, at, label);
  }
}

export function deriveThreadInfo(
  items: readonly HarnessItem[],
  { threadId, lectureId }: { threadId: number; lectureId?: string | null },
): ThreadInfoLists {
  const lists = new Lists();
  // The lecture a lecture thread is about leads Read before any message.
  if (lectureId) lists.addRead(recordingRel(lectureId), -1, { pinned: true });
  items.forEach((item, at) => {
    if (item.thread_id !== threadId) return;
    if (item.kind === "user" || item.kind === "steer") {
      for (const part of splitLibraryPaths(item.content ?? "")) {
        if (part.kind === "path") {
          lists.addRead(part.path, at, {
            pages: part.cite.page ? [{ from: part.cite.page, to: part.cite.page }] : [],
            time: part.cite.at,
          });
        } else if (part.kind === "attachment") {
          lists.addRead(part.path, at);
        }
      }
      const moment = lectureId ? messageAt(item) : null;
      if (moment != null && lectureId) lists.addRead(recordingRel(lectureId), at, { time: moment });
      return;
    }
    if (item.kind !== "tool") return;
    const meta = parseToolMeta(item);
    // A refused or failed call read and made nothing.
    if (meta.ok === false) return;
    const input = (meta.input && typeof meta.input === "object" ? meta.input : {}) as Record<string, unknown>;
    switch (meta.kind) {
      case "read":
        for (const raw of toolPaths(input)) {
          const lib = toLibraryPath(raw);
          if (lib) lists.addRead(lib, at, { pages: parsePages(str(input.pages)) });
        }
        return;
      case "write":
      case "edit":
        for (const raw of toolPaths(input)) lists.addMade(raw, at, meta.kind === "write" ? "new" : "edited");
        return;
      case "bash":
      case "oculus_cli": {
        const line = str(input.command) ?? str(input.CommandLine) ?? item.content ?? "";
        scanCommand(line, at, lists);
        return;
      }
    }
  });
  return lists.done();
}
