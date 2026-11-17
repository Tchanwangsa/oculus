import { describe, expect, test } from "bun:test";

import { caretChunk, chunksText, maskedText, normalize, type Chunk } from "@/components/harness/mentions/chunks";
import { detectTrigger, isPathWord, slashQuery } from "@/components/harness/mentions/trigger";
import { composerCommands } from "@/lib/harness/composerCommands";
import type { MentionFile } from "@/lib/db";
import type { Skill } from "@/lib/harness/skills";

// `dom.ts` and the menu hook reach modules that listen on `window` and
// `document` at load; they get inert stand-ins for the import, then the
// globals go back so no other test sees a half DOM.
const globals = globalThis as { window?: unknown; document?: unknown };
const before = { window: globals.window, document: globals.document };
const inert = { addEventListener: () => {}, removeEventListener: () => {} };
globals.window ??= inert;
globals.document ??= { ...inert, documentElement: { style: {} } };
const { toChunks } = await import("@/components/harness/mentions/dom");
const { menuContent, menuDirection, pickable } = await import("@/components/harness/mentions/useComposerMenu");
Object.assign(globals, before);

const at = (text: string) => detectTrigger(text, text.length);

describe("menuDirection", () => {
  test("a bottom composer opens up even when its empty-state line would fit below", () => {
    expect(menuDirection({ left: 200, top: 760, bottom: 780 }, 880)).toBe("up");
  });

  test("a mid-page composer opens down when a full menu fits", () => {
    expect(menuDirection({ left: 200, top: 400, bottom: 420 }, 880)).toBe("down");
  });

  test("when neither side fits a full menu, it uses the roomier side", () => {
    expect(menuDirection({ left: 20, top: 170, bottom: 190 }, 300)).toBe("up");
    expect(menuDirection({ left: 20, top: 100, bottom: 120 }, 300)).toBe("down");
  });
});

describe("detectTrigger", () => {
  test.each(["1/2", "/usr/bin", "a/b", "$x$", "and/or", "see /etc/hosts", "https://example.com/review", "Use /review "])(
    "never opens a menu for %j",
    (text) => expect(at(text)).toBeNull(),
  );

  test("a slash at the start of a word opens the slash menu", () => {
    expect(at("/")).toEqual({ kind: "slash", query: "", start: 0 });
    expect(at("Use /rev")).toEqual({ kind: "slash", query: "rev", start: 4 });
    expect(at("line one\n/mo")).toEqual({ kind: "slash", query: "mo", start: 9 });
  });

  test("a second slash past the caret still makes the word a path", () => {
    const text = "Use /etc/hosts now";
    expect(detectTrigger(text, "Use /et".length)).toBeNull();
    expect(isPathWord(text, 4, 7)).toBe(true);
  });

  test("@ keeps its multi-word query", () => {
    expect(at("read @week 3 notes")).toEqual({ kind: "mention", query: "week 3 notes", start: 5 });
    expect(at("mail me@home")).toBeNull();
  });

  test("a slash word wins over an @ query that ran on past it", () => {
    expect(at("@notes /mo")).toEqual({ kind: "slash", query: "mo", start: 7 });
  });

  test("a picked skill, masked, never reopens the menu", () => {
    const chunks: Chunk[] = [{ kind: "skill", name: "oculus-plan" }];
    const text = chunksText(normalize(chunks));
    expect(text).toBe("/oculus-plan");
    expect(slashQuery(text, text.length)).not.toBeNull();
    expect(slashQuery(maskedText(normalize(chunks)), text.length)).toBeNull();
  });
});

describe("skill chunks", () => {
  const known = new Set(["oculus-plan"]);

  test("round-trip through the draft as /name", () => {
    const chunks = normalize(toChunks("/oculus-plan for week 3", known));
    expect(chunks.filter((c) => c.kind !== "text")).toEqual([{ kind: "skill", name: "oculus-plan" }]);
    expect(chunksText(chunks)).toBe("/oculus-plan for week 3");
  });

  test("a name the catalogue lacks stays text", () => {
    expect(normalize(toChunks("/other thing", known))).toEqual([{ kind: "text", text: "/other thing" }]);
    expect(normalize(toChunks("/oculus-plan", undefined))).toEqual([{ kind: "text", text: "/oculus-plan" }]);
  });

  test("a leading chip gets a caret guard, and an offset inside lands after it", () => {
    const chunks = normalize(toChunks("/oculus-plan go", known));
    expect(chunks[0].kind).toBe("text");
    expect(chunks[1]).toEqual({ kind: "skill", name: "oculus-plan" });
    expect(caretChunk(chunks, 3)).toEqual({ chunk: 2, offset: 0 });
  });

  test("a folder chip round-trips with its trailing slash", () => {
    const text = "list `courses/MAST20004/Course/Pages/` please";
    const chunks = normalize(toChunks(text, known));
    expect(chunks.filter((c) => c.kind === "chip")).toEqual([{ kind: "chip", path: "courses/MAST20004/Course/Pages/" }]);
    expect(chunksText(chunks)).toBe(text);
  });

  test("mentions and skills share a sentence", () => {
    const text = "/oculus-plan using `courses/MAST20004/Course/Pages/week-1.md`";
    const chunks = normalize(toChunks(text, known));
    expect(chunks.map((c) => c.kind)).toEqual(["text", "skill", "text", "chip"]);
    expect(chunksText(chunks)).toBe(text);
  });
});

describe("menuContent", () => {
  const skill = (name: string, description = ""): Skill => ({
    name,
    description,
    path: `/s/${name}/SKILL.md`,
    source: "oculus",
    userInvocable: true,
  });
  const commands = composerCommands();
  const file = { id: 1, subject_id: 1, subject_code: "M", filename: "a.md", relative_path: "courses/M/Course/Pages/a.md", category: "page" } as MentionFile;

  test("groups commands then skills", () => {
    const { groups, note } = menuContent({ trigger: { kind: "slash", query: "", start: 0 }, library: null, slow: false, commands, skills: [skill("oculus-plan")] });
    expect(groups.map((g) => [g.label, g.rows.length])).toEqual([["Commands", 1], ["Skills", 1]]);
    expect(note).toBeNull();
  });

  test("says when skills are loading and when nothing matches", () => {
    const loading = menuContent({ trigger: { kind: "slash", query: "", start: 0 }, library: null, slow: false, commands, skills: null });
    expect(loading.note).toBe("Searching skills…");
    expect(loading.groups.map((g) => g.label)).toEqual(["Commands"]);
    const none = menuContent({ trigger: { kind: "slash", query: "zzzz", start: 0 }, library: null, slow: false, commands, skills: [] });
    expect(none).toEqual({ groups: [], note: "No matching command.", unparsed: 0 });
  });

  test("lists folders before files on @", () => {
    const folder = { path: "courses/M/Course/Pages/", name: "pages", subject_id: 1, subject_code: "M" };
    const { groups } = menuContent({ trigger: { kind: "mention", query: "", start: 0 }, library: { folders: [folder], files: [file], unparsed: 0 }, slow: false, commands, skills: [] });
    expect(groups[0].rows.map((i) => i.kind)).toEqual(["folder", "file"]);
  });

  test("an opened folder shows its subfolders, then its files, one level in", () => {
    const folder = (path: string, name: string) => ({ path, name, subject_id: 1, subject_code: "M" });
    const files = folder("courses/M/Course/Files/", "files");
    const week = folder("courses/M/Course/Files/Week_1/", "Week_1");
    const inside = { ...file, id: 2, relative_path: "courses/M/Course/Files/syllabus.pdf" };
    const deep = { ...file, id: 3, relative_path: "courses/M/Course/Files/Week_1/notes.pdf" };
    const rows = (opened: Map<string, { folders: (typeof files)[]; files: MentionFile[]; more: number }>) =>
      menuContent({ trigger: { kind: "mention", query: "", start: 0 }, library: { folders: [files], files: [file], unparsed: 0 }, slow: false, commands, skills: [], opened })
        .groups[0].rows.map((r) => (r.kind === "line" ? `${r.depth} ${r.text}` : r.kind === "folder" ? `${r.depth} ${r.folder.name}${r.open ? " open" : ""}` : r.kind === "file" ? `${r.depth} #${r.file.id}` : ""));

    expect(rows(new Map())).toEqual(["0 files", "0 #1"]);
    const one = new Map([[files.path, { folders: [week], files: [inside], more: 0 }]]);
    expect(rows(one)).toEqual(["0 files open", "1 Week_1", "1 #2", "0 #1"]);
    const two = new Map(one).set(week.path, { folders: [], files: [deep], more: 40 });
    expect(rows(two)).toEqual(["0 files open", "1 Week_1 open", "2 #3", "2 40 more. Type a name to find one.", "1 #2", "0 #1"]);
    const empty = new Map([[files.path, { folders: [], files: [], more: 0 }]]);
    expect(rows(empty)).toEqual(["0 files open", "1 Nothing here to mention.", "0 #1"]);
    // A line is never picked: the highlight and Enter move over the rest.
    expect(pickable(menuContent({ trigger: { kind: "mention", query: "", start: 0 }, library: { folders: [files], files: [file], unparsed: 0 }, slow: false, commands, skills: [], opened: empty }).groups[0].rows)).toHaveLength(2);
  });

  test("says it is searching only once a lookup is slow", () => {
    const trigger = { kind: "mention" as const, query: "wee", start: 0 };
    expect(menuContent({ trigger, library: null, slow: false, commands, skills: [] }).note).toBeNull();
    expect(menuContent({ trigger, library: null, slow: true, commands, skills: [] }).note).toBe("Searching the library…");
  });

  test("an empty @ lookup says so for one word, and stays shut for prose", () => {
    const empty = { folders: [], files: [], unparsed: 0 };
    expect(menuContent({ trigger: { kind: "mention", query: "wee", start: 0 }, library: empty, slow: false, commands, skills: [] }).note).toBe("No matching files or folders.");
    expect(menuContent({ trigger: { kind: "mention", query: "home I was", start: 0 }, library: empty, slow: false, commands, skills: [] }).note).toBeNull();
    expect(menuContent({ trigger: { kind: "mention", query: "wee", start: 0 }, library: { ...empty, unparsed: 2 }, slow: false, commands, skills: [] })).toEqual({ groups: [], note: null, unparsed: 2 });
  });
});
