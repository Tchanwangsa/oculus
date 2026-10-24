import { displayCode, humanizeSlug } from "@/lib/format/format";

/** What a timeline row says about an `oculus …` tool call: "Saved a memory",
 *  "Searched the library for "word limit"", "Moved 7 tasks to To do". The
 *  command stays in the row's detail; this is only its title. */

export type CliIcon =
  | "memory"
  | "search"
  | "file"
  | "folder"
  | "calendar"
  | "task"
  | "project"
  | "tag"
  | "lecture"
  | "studio"
  | "library";

/** One `oculus` command out of a shell line. */
export interface CliAction {
  /** Same key, same command: chains of it are counted. */
  key: string;
  icon: CliIcon;
  em?: string;
  verb: (done: boolean, n: number) => string;
  /** The bundle's clause for `n` of these: "saved 2 memories". */
  tally: (n: number) => string;
}

export interface CliSummary {
  title: string;
  em?: string;
  icon: CliIcon;
}

interface Args {
  pos: string[];
  flags: Map<string, string | true>;
}

/** Flags that take no value, so the word after them is positional. */
const BOOL_FLAGS = new Set([
  "--all", "--full", "--fixed", "--case-sensitive", "-l", "--files-with-matches",
  "--indexed", "--due", "--past", "--reparse", "--refresh", "--ed", "--no-parse",
  "--no-embed", "--videos", "--transcripts", "--force", "--stdout", "--global",
  "--keep", "--personal", "--archived", "--unfiled", "--frames", "--dry-run",
  "--print-prompt", "--json", "--help", "-h", "-i", "--subjects", "--lectures",
]);

const SEPARATORS = ["&&", "||", ";", "\n"];

/** The commands of a shell line as words, quotes removed. A pipe or redirect
 *  ends what is kept of a command (`| head -30`, `2>&1`). */
export function shellCommands(line: string): string[][] {
  const out: string[][] = [];
  let words: string[] = [];
  let word = "";
  let inWord = false;
  let quote: string | null = null;
  let dropping = false;
  const endWord = () => {
    if (inWord && !dropping) words.push(word);
    word = "";
    inWord = false;
  };
  const endCommand = () => {
    endWord();
    if (words.length) out.push(words);
    words = [];
    dropping = false;
  };
  for (let i = 0; i < line.length; i++) {
    const c = line[i];
    if (quote) {
      if (c === quote) quote = null;
      else if (c === "\\" && quote === '"' && i + 1 < line.length) word += line[++i];
      else word += c;
      continue;
    }
    const sep = SEPARATORS.find((s) => line.startsWith(s, i));
    if (sep) {
      endCommand();
      i += sep.length - 1;
    } else if (c === "'" || c === '"') {
      quote = c;
      inWord = true;
    } else if (c === "\\" && i + 1 < line.length) {
      word += line[++i];
      inWord = true;
    } else if (c === "|" || c === ">" || c === "<") {
      // `2>&1`: the fd number is part of the redirect, not an argument.
      if (/^\d+$/.test(word)) {
        word = "";
        inWord = false;
      }
      endWord();
      dropping = true;
    } else if (/\s/.test(c)) {
      endWord();
    } else {
      word += c;
      inWord = true;
    }
  }
  endCommand();
  return out;
}

const isOculus = (w: string) => /^oculus(\.exe)?$/i.test(w.split(/[/\\]/).pop() ?? "");

function parseArgs(words: string[]): Args {
  const pos: string[] = [];
  const flags = new Map<string, string | true>();
  for (let i = 0; i < words.length; i++) {
    const w = words[i];
    if (!w.startsWith("-") || w === "-") {
      pos.push(w);
      continue;
    }
    const eq = w.indexOf("=");
    if (w.startsWith("--") && eq > 0) {
      flags.set(w.slice(0, eq), w.slice(eq + 1));
    } else if (!BOOL_FLAGS.has(w) && i + 1 < words.length && !words[i + 1].startsWith("-")) {
      flags.set(w, words[++i]);
    } else {
      flags.set(w, true);
    }
  }
  return { pos, flags };
}

function flag(a: Args, ...names: string[]): string | undefined {
  for (const n of names) {
    const v = a.flags.get(n);
    if (typeof v === "string" && v) return v;
  }
  return undefined;
}

const plural = (n: number, one: string, many = `${one}s`) => (n === 1 ? one : many);
const base = (p: string) => p.replace(/[/\\]+$/, "").split(/[/\\]/).pop() ?? p;
const code = (c: string | undefined) => (c ? displayCode(c) : undefined);
const join = (...parts: (string | undefined)[]) => parts.filter(Boolean).join(" · ") || undefined;

/** `todo` → "To do", `in-review` → "In review". */
function column(c: string): string {
  if (c.toLowerCase() === "todo") return "To do";
  const s = c.replace(/[-_]+/g, " ");
  return s.charAt(0).toUpperCase() + s.slice(1);
}

/** The course a library path sits under, `courses/COMP30026_2026_SM2/…`. */
function pathCourse(p: string): string | undefined {
  return /(?:^|[/\\])courses[/\\]([^/\\]+)/.exec(p)?.[1];
}

/** A verb with its live form, and how a chain of `n` reads. */
function verbs(live: string, done: string, many?: (n: number) => [string, string]) {
  return (isDone: boolean, n: number) => {
    if (n > 1 && many) return many(n)[isDone ? 1 : 0];
    return isDone ? done : live;
  };
}

const once = (clause: string) => () => clause;
const counted = (one: string, many: (n: number) => string) => (n: number) => (n === 1 ? one : many(n));

const SEARCH_TALLY = counted("searched the library", (n) => `searched the library ${n} times`);

function describeOne(sub: string[], a: Args): CliAction {
  const [cmd, action] = sub;
  const name = a.pos[0];
  switch (cmd) {
    case "memory":
      switch (action) {
        case "write":
          return {
            key: "memory-write", icon: "memory",
            em: flag(a, "--about") ?? (name ? humanizeSlug(name) : undefined),
            verb: verbs("Saving a memory", "Saved a memory", (n) => [`Saving ${n} memories`, `Saved ${n} memories`]),
            tally: counted("saved a memory", (n) => `saved ${n} memories`),
          };
        case "append":
          return {
            key: "memory-write", icon: "memory", em: name ? humanizeSlug(name) : undefined,
            verb: verbs("Saving a memory", "Saved a memory", (n) => [`Saving ${n} memories`, `Saved ${n} memories`]),
            tally: counted("saved a memory", (n) => `saved ${n} memories`),
          };
        case "fold": {
          const into = flag(a, "--into");
          return {
            key: "memory-fold", icon: "memory", em: into ? humanizeSlug(into) : undefined,
            verb: verbs("Folding memories into", "Folded memories into"),
            tally: once("folded memories"),
          };
        }
        case "read":
          return {
            key: "memory-read", icon: "memory", em: name,
            verb: verbs("Reading memory", "Read memory", (n) => [`Reading ${n} memories`, `Read ${n} memories`]),
            tally: counted("read a memory", (n) => `read ${n} memories`),
          };
        case "list":
          return {
            key: "memory-list", icon: "memory",
            em: a.flags.has("--all") ? "all subjects" : code(flag(a, "-s", "--subject")),
            verb: verbs("Listing memories", "Listed memories"),
            tally: once("listed memories"),
          };
        case "rm":
          return {
            key: "memory-rm", icon: "memory", em: name,
            verb: verbs("Deleting memory", "Deleted memory", (n) => [`Deleting ${n} memories`, `Deleted ${n} memories`]),
            tally: counted("deleted a memory", (n) => `deleted ${n} memories`),
          };
        case "move":
        case "promote":
          return {
            key: "memory-move", icon: "memory", em: name,
            verb: verbs("Moving memory", "Moved memory", (n) => [`Moving ${n} memories`, `Moved ${n} memories`]),
            tally: counted("moved a memory", (n) => `moved ${n} memories`),
          };
        case "reindex":
          return { key: "memory-reindex", icon: "memory", verb: verbs("Reindexing memories", "Reindexed memories"), tally: once("reindexed memories") };
      }
      break;
    case "grep":
    case "search":
      return {
        key: "search", icon: "search",
        em: join(name ? `"${name}"` : undefined, code(flag(a, "-s", "--subject"))),
        verb: verbs("Searching the library for", "Searched the library for", (n) => [
          `Searching the library ${n} times`, `Searched the library ${n} times`,
        ]),
        tally: SEARCH_TALLY,
      };
    case "read": {
      const pages = flag(a, "-p", "--pages");
      return {
        key: "read", icon: "file",
        em: name ? join(base(name), pages && `p. ${pages}`, code(pathCourse(name) ?? flag(a, "-s", "--subject"))) : undefined,
        verb: verbs("Reading", "Read", (n) => [`Reading ${n} files`, `Read ${n} files`]),
        tally: counted("read a library file", (n) => `read ${n} library files`),
      };
    }
    case "files":
      return {
        key: "files", icon: "folder", em: a.pos.map(displayCode).join(", ") || undefined,
        verb: verbs("Listing files", "Listed files"), tally: once("listed files"),
      };
    case "calendar":
      return { key: "calendar", icon: "calendar", verb: verbs("Checking the calendar", "Checked the calendar"), tally: once("checked the calendar") };
    case "list": {
      const what = a.flags.has("-l") || a.flags.has("--lectures") ? "lectures" : "subjects";
      return { key: `list-${what}`, icon: "library", verb: verbs(`Listing ${what}`, `Listed ${what}`), tally: once(`listed ${what}`) };
    }
    case "status":
      return { key: "status", icon: "library", verb: verbs("Checking status", "Checked status"), tally: once("checked status") };
    case "task":
      return describeTask(action, a);
    case "project":
      return describeProject(action, a);
    case "tag":
      return action === "list"
        ? { key: "tag-list", icon: "tag", verb: verbs("Listing tags", "Listed tags"), tally: once("listed tags") }
        : { key: "tag", icon: "tag", em: name, verb: verbs("Updating tags", "Updated tags"), tally: once("updated tags") };
    case "lecture":
      return { key: "lecture", icon: "lecture", em: action, verb: verbs("Looking inside a lecture", "Looked inside a lecture"), tally: once("looked inside a lecture") };
    case "transcribe":
      return { key: "transcribe", icon: "lecture", em: name && base(name), verb: verbs("Transcribing", "Transcribed"), tally: once("transcribed a recording") };
    case "studio":
      return describeStudio(action);
    case "docs":
      return { key: "docs", icon: "library", em: name, verb: verbs("Reading the Oculus docs", "Read the Oculus docs"), tally: once("read the Oculus docs") };
    case "run":
      return { key: "run", icon: "library", verb: verbs("Syncing Canvas", "Synced Canvas"), tally: once("synced Canvas") };
  }
  return {
    key: `other-${sub.join(" ")}`, icon: "library",
    em: sub.filter(Boolean).join(" ") || undefined,
    verb: verbs("Running oculus", "Ran oculus"),
    tally: counted("ran an oculus command", (n) => `ran ${n} oculus commands`),
  };
}

function describeTask(action: string | undefined, a: Args): CliAction {
  const id = a.pos[0] ? `#${a.pos[0]}` : undefined;
  const tasks = (n: number) => `${n} ${plural(n, "task")}`;
  switch (action) {
    case "add":
      return {
        key: "task-add", icon: "task", em: flag(a, "--title") ?? a.pos[0],
        verb: verbs("Adding a task", "Added a task", (n) => [`Adding ${tasks(n)}`, `Added ${tasks(n)}`]),
        tally: counted("added a task", (n) => `added ${tasks(n)}`),
      };
    case "move": {
      const col = flag(a, "-c", "--column");
      return {
        key: "task-move", icon: "task", em: col ? column(col) : id,
        verb: verbs(col ? "Moving task to" : "Moving task", col ? "Moved task to" : "Moved task", (n) => [
          `Moving ${tasks(n)}${col ? " to" : ""}`, `Moved ${tasks(n)}${col ? " to" : ""}`,
        ]),
        tally: counted("moved a task", (n) => `moved ${tasks(n)}`),
      };
    }
    case "update":
      return {
        key: "task-update", icon: "task", em: flag(a, "--title") ?? id,
        verb: verbs("Updating task", "Updated task", (n) => [`Updating ${tasks(n)}`, `Updated ${tasks(n)}`]),
        tally: counted("updated a task", (n) => `updated ${tasks(n)}`),
      };
    case "rm":
      return {
        key: "task-rm", icon: "task", em: id,
        verb: verbs("Deleting task", "Deleted task", (n) => [`Deleting ${tasks(n)}`, `Deleted ${tasks(n)}`]),
        tally: counted("deleted a task", (n) => `deleted ${tasks(n)}`),
      };
    case "refile":
      return { key: "task-refile", icon: "task", em: id, verb: verbs("Refiling task", "Refiled task"), tally: counted("refiled a task", (n) => `refiled ${tasks(n)}`) };
    case "show":
      return { key: "task-show", icon: "task", em: id, verb: verbs("Opening task", "Opened task"), tally: counted("opened a task", (n) => `opened ${tasks(n)}`) };
    default:
      return { key: "task-list", icon: "task", verb: verbs("Listing tasks", "Listed tasks"), tally: once("listed tasks") };
  }
}

function describeProject(action: string | undefined, a: Args): CliAction {
  const name = flag(a, "--title", "--name") ?? a.pos[0];
  switch (action) {
    case "create":
      return { key: "project-create", icon: "project", em: name, verb: verbs("Creating project", "Created project"), tally: counted("created a project", (n) => `created ${n} projects`) };
    case "update":
      return { key: "project-update", icon: "project", em: name, verb: verbs("Updating project", "Updated project"), tally: counted("updated a project", (n) => `updated ${n} projects`) };
    case "show":
      return { key: "project-show", icon: "project", em: name, verb: verbs("Opening project", "Opened project"), tally: counted("opened a project", (n) => `opened ${n} projects`) };
    case "events":
      return { key: "project-events", icon: "project", em: name, verb: verbs("Reading project history", "Read project history"), tally: once("read project history") };
    default:
      return { key: "project-list", icon: "project", verb: verbs("Listing projects", "Listed projects"), tally: once("listed projects") };
  }
}

function describeStudio(action: string | undefined): CliAction {
  const studio = (live: string, done: string): CliAction => ({
    key: `studio-${action}`, icon: "studio", verb: verbs(live, done),
    tally: counted("ran a Studio step", (n) => `ran ${n} Studio steps`),
  });
  switch (action) {
    case "render": return studio("Rendering the video", "Rendered the video");
    case "frame": return studio("Rendering a frame", "Rendered a frame");
    case "narrate": return studio("Narrating", "Narrated");
    case "lint": return studio("Checking the scenes", "Checked the scenes");
    default: return { ...studio("Running Studio", "Ran Studio"), em: action };
  }
}

/** Every `oculus` command in a shell line, in order; other commands in the
 *  line (`cd`, a trailing `head`) are skipped. */
export function describeOculusCli(line: string): CliAction[] {
  const out: CliAction[] = [];
  for (const words of shellCommands(line)) {
    // `oculus` or a path to it, within the first three words like Rust's
    // `is_oculus_cli` (behind `VAR=x` or `time`).
    const at = words.findIndex((w, i) => i < 3 && isOculus(w));
    if (at < 0) continue;
    const rest = words.slice(at + 1);
    const sub: string[] = [];
    // Command, then an action word for the families that have them.
    if (rest[0] && !rest[0].startsWith("-")) sub.push(rest.shift()!);
    if (["memory", "task", "project", "tag", "lecture", "studio"].includes(sub[0]) && rest[0] && !rest[0].startsWith("-")) {
      sub.push(rest.shift()!);
    }
    out.push(describeOne(sub, parseArgs(rest)));
  }
  return out;
}

/** The row's title for a line: a chain of one command is counted ("Moved 7
 *  tasks to"); a mixed chain is titled by its first. */
export function summariseOculusCli(actions: CliAction[], done: boolean): CliSummary | null {
  const first = actions[0];
  if (!first) return null;
  const same = actions.every((x) => x.key === first.key);
  const n = same ? actions.length : 1;
  const ems = [...new Set((same ? actions : [first]).map((x) => x.em).filter((e): e is string => !!e))];
  const em = ems.join(", ") || undefined;
  return { title: first.verb(done, n), em: same ? em : join(em, `+${actions.length - 1} more`), icon: first.icon };
}
