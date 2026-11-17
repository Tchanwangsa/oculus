import { describe, expect, test } from "bun:test";
import {
  deriveThreadInfo,
  formatPages,
  formatTimes,
  mergePages,
  parsePages,
  studioHref,
} from "@/lib/harness/threadInfo";
import type { HarnessItem } from "@/lib/harness";

const DATA = "/Users/x/Library/Application Support/com.tchan.oculus";
let nextId = 1;

function row(kind: HarnessItem["kind"], content: string | null, meta?: object, thread = 7): HarnessItem {
  return {
    id: nextId++,
    thread_id: thread,
    kind,
    ref_id: null,
    content,
    meta: meta ? JSON.stringify(meta) : null,
    created_at: "2026-11-16 10:00:00",
  };
}

const tool = (kind: string, input: object, ok: boolean | null = true, content: string | null = null) =>
  row("tool", content, { kind, name: kind, input, ok });

const info = (items: HarnessItem[], lectureFolder?: string) => deriveThreadInfo(items, { threadId: 7, lectureFolder });

describe("page ranges", () => {
  test("parses single pages, spans, lists and open ends", () => {
    expect(parsePages("12")).toEqual([{ from: 12, to: 12 }]);
    expect(parsePages("12-15")).toEqual([{ from: 12, to: 15 }]);
    expect(parsePages("12,14,20-22")).toEqual([
      { from: 12, to: 12 },
      { from: 14, to: 14 },
      { from: 20, to: 22 },
    ]);
    expect(parsePages("30-")).toEqual([{ from: 30, to: null }]);
    expect(parsePages("0, x, 5")).toEqual([{ from: 5, to: 5 }]);
    expect(parsePages(null)).toEqual([]);
  });

  test("merges overlapping and touching ranges, and an open end swallows what follows", () => {
    expect(mergePages(parsePages("16,12-15,13"))).toEqual([{ from: 12, to: 16 }]);
    expect(mergePages(parsePages("3,12-19"))).toEqual([
      { from: 3, to: 3 },
      { from: 12, to: 19 },
    ]);
    expect(mergePages(parsePages("30-,40-41,28"))).toEqual([{ from: 28, to: 28 }, { from: 30, to: null }]);
  });

  test("formats as p. and pp.", () => {
    expect(formatPages(parsePages("12"))).toBe("p. 12");
    expect(formatPages(parsePages("12-19"))).toBe("pp. 12–19");
    expect(formatPages(parsePages("12-19,3"))).toBe("pp. 3, 12–19");
    expect(formatPages(parsePages("30-"))).toBe("pp. 30–");
    expect(formatPages([])).toBe("");
  });

  test("lecture moments read as a time or the span they cover", () => {
    expect(formatTimes([740])).toBe("12:20");
    expect(formatTimes([1865, 740, 900])).toBe("12:20–31:05");
    expect(formatTimes([])).toBe("");
  });
});

describe("Read", () => {
  test("one row per file: a parsed .md is its PDF, and pages from every read merge", () => {
    const { read } = info([
      row("tool", "oculus read", { kind: "oculus_cli", input: { command: "oculus read courses/COMP30024/Course/Files/week-7-search.pdf -p 12-15" }, ok: true }),
      tool("read", { file_path: `${DATA}/courses/COMP30024/Course/Files/week-7-search.md` }),
      row("tool", "oculus read", { kind: "oculus_cli", input: { command: "oculus read courses/COMP30024/Course/Files/week-7-search.pdf --pages 16-19" }, ok: true }),
    ]);
    expect(read).toHaveLength(1);
    expect(read[0].key).toBe("courses/COMP30024/Course/Files/week-7-search.pdf");
    expect(formatPages(read[0].pages)).toBe("pp. 12–19");
    expect(read[0].target).toEqual({ kind: "library", path: "courses/COMP30024/Course/Files/week-7-search.pdf", page: 12 });
  });

  test("most recently touched first; touching again moves a file up", () => {
    const { read } = info([
      tool("read", { file_path: "../courses/A/Course/Pages/one.md" }),
      tool("read", { file_path: "../courses/A/Course/Pages/two.md" }),
      tool("read", { file_path: "../courses/A/Course/Pages/one.md" }),
    ]);
    expect(read.map((e) => e.key)).toEqual(["courses/A/Course/Pages/one.md", "courses/A/Course/Pages/two.md"]);
  });

  test("@ mentions and attachments in the student's messages and steers count", () => {
    const { read } = info([
      row("user", "Explain `courses/A/Course/Files/slides.pdf#page=4` and `./attachments/notes.pdf`"),
      row("steer", "also `courses/A/Course/Pages/`"),
    ]);
    expect(read.map((e) => e.key)).toEqual([
      "courses/A/Course/Pages/",
      "courses/A/Course/Files/slides.pdf",
      "agents/attachments/notes.pdf",
    ]);
    expect(formatPages(read[1].pages)).toBe("p. 4");
  });

  test("a shell reader's library paths count; the agent's own skills and memories do not", () => {
    const { read } = info([
      tool("bash", { command: "sed -n 1,80p ../courses/A/Course/Pages/brief.md | head -5" }),
      tool("read", { file_path: `${DATA}/agents/.claude/skills/oculus-library/SKILL.md` }),
      tool("read", { file_path: `${DATA}/agents/memories/MEMORY.md` }),
      tool("bash", { command: "cat AGENTS.md" }),
    ]);
    expect(read.map((e) => e.key)).toEqual(["courses/A/Course/Pages/brief.md"]);
  });

  test("failed and refused calls read nothing; a running read counts", () => {
    const { read } = info([
      tool("read", { file_path: "../courses/A/Course/Pages/denied.md" }, false),
      tool("read", { file_path: "../courses/A/Course/Pages/running.md" }, null),
    ]);
    expect(read.map((e) => e.key)).toEqual(["courses/A/Course/Pages/running.md"]);
  });

  test("a lecture thread's lecture leads, with the moments asked at", () => {
    const { read } = info(
      [
        tool("read", { file_path: "../courses/A/Course/Files/deck.pdf" }),
        row("user", "what is this?", { at: 740 }),
        tool("read", { file_path: `${DATA}/courses/A/Course/Recordings/2026-08-04-lec-1/transcript.vtt` }),
        row("user", "and this?", { at: 1865 }),
        tool("read", { file_path: "../courses/A/Course/Pages/after.md" }),
      ],
      "courses/A/Course/Recordings/2026-08-04-lec-1",
    );
    expect(read.map((e) => e.key)).toEqual(["courses/A/Course/Recordings/2026-08-04-lec-1", "courses/A/Course/Pages/after.md", "courses/A/Course/Files/deck.pdf"]);
    expect(read[0].pinned).toBe(true);
    expect(formatTimes(read[0].times)).toBe("12:20–31:05");
    expect(read[0].target).toEqual({ kind: "library", path: "courses/A/Course/Recordings/2026-08-04-lec-1", at: 1865 });
  });

  test("the lecture shows before any message, and other threads' rows are ignored", () => {
    const { read } = info([row("user", "`courses/B/Course/Pages/x.md`", undefined, 8)], "courses/A/Course/Recordings/2026-08-05-lec-2");
    expect(read.map((e) => e.key)).toEqual(["courses/A/Course/Recordings/2026-08-05-lec-2"]);
  });
});

describe("Made", () => {
  test("writes and edits, deduped, first touch decides new or edited", () => {
    const { made } = info([
      tool("write", { file_path: `${DATA}/agents/outputs/summary.md` }),
      tool("edit", { file_path: "./outputs/summary.md" }),
      tool("edit", { paths: [`${DATA}/agents/outputs/plot.py`] }),
    ]);
    expect(made.map((e) => [e.key, e.made])).toEqual([
      ["agents/outputs/plot.py", "edited"],
      ["agents/outputs/summary.md", "new"],
    ]);
  });

  test("a file it made is not also listed as read", () => {
    const { read, made } = info([
      tool("read", { file_path: "./outputs/summary.md" }),
      tool("edit", { file_path: "./outputs/summary.md" }),
    ]);
    expect(read).toEqual([]);
    expect(made.map((e) => e.key)).toEqual(["agents/outputs/summary.md"]);
  });

  test("a shell line's outputs/ files are made, unless it only reads them", () => {
    const { made, read } = info([
      tool("bash", { command: "python plot.py > outputs/curve.png && echo ok" }),
      tool("bash", { command: "python make.py --out ./outputs/table.csv" }),
      tool("bash", { command: "cat outputs/old.md" }),
      tool("bash", { command: "ls outputs/" }),
    ]);
    expect(made.map((e) => e.key)).toEqual(["agents/outputs/table.csv", "agents/outputs/curve.png"]);
    expect(read.map((e) => e.key)).toEqual(["agents/outputs/old.md"]);
  });

  test("oculus commands that write: a memory opens its file, a render its video", () => {
    const { made } = info([
      row("tool", "oculus memory write exam-tips -s COMP30024 --about x", {
        kind: "oculus_cli", input: { command: "oculus memory write exam-tips -s COMP30024 --about x" }, ok: true,
      }),
      row("tool", "oculus memory write --about y", {
        kind: "oculus_cli", input: { command: "oculus memory write --about y --text z" }, ok: true,
      }),
      row("tool", "", {
        kind: "oculus_cli", input: { command: `oculus studio render "${DATA}/studio/videos/minimax" --final` }, ok: true,
      }),
      row("tool", "", { kind: "oculus_cli", input: { command: "oculus task add --title hi" }, ok: false }),
      row("tool", "", { kind: "oculus_cli", input: { command: "oculus search alpha" }, ok: true }),
    ]);
    expect(made.map((e) => [e.key, e.target, e.label])).toEqual([
      ["/studio/videos/minimax#render", { kind: "route", href: "/studio/videos/minimax" }, "Rendered the video"],
      ["agents/memories/", { kind: "library", path: "agents/memories/" }, undefined],
      ["agents/memories/COMP30024/exam-tips.md", { kind: "library", path: "agents/memories/COMP30024/exam-tips.md" }, undefined],
    ]);
  });

  test("Studio files open their video, component or theme; the agent's instructions are not made", () => {
    const { made } = info([
      tool("write", { file_path: `${DATA}/studio/videos/minimax/script.md` }),
      tool("write", { file_path: `${DATA}/studio/components/PrunedLeaf.tsx` }),
      tool("edit", { file_path: `${DATA}/agents/AGENTS.md` }),
      tool("write", { file_path: "/tmp/scratch.txt" }),
    ]);
    expect(made.map((e) => e.target)).toEqual([
      { kind: "route", href: "/studio/components/PrunedLeaf" },
      { kind: "route", href: "/studio/videos/minimax" },
    ]);
    expect(studioHref("/tmp/studio/videos/x")).toBeNull();
  });

  test("an apply_patch envelope makes every file it adds or updates", () => {
    const patchText = [
      "*** Begin Patch",
      `*** Add File: ${DATA}/agents/outputs/a.md`,
      "+hi",
      `*** Update File: ${DATA}/agents/outputs/b.md`,
      `*** Delete File: ${DATA}/agents/outputs/c.md`,
      "*** End Patch",
    ].join("\n");
    const { made } = info([tool("edit", { patchText })]);
    expect(made.map((e) => e.key).sort()).toEqual(["agents/outputs/a.md", "agents/outputs/b.md"]);
  });
});
