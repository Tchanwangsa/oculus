import { describe, expect, test } from "bun:test";
import type { DbFile } from "@/lib/db";
import {
  OTHER_GROUP,
  defaultSlide,
  groupLessons,
  lessonProgress,
  parseEdLesson,
  parseLessonIndex,
  parseSlideChallenge,
  resolveBeside,
  slideFiles,
  slideIcon,
  lessonIcon,
} from "@/lib/ed/lessons";
import { Code, File, FileCode, FilePy, FileText } from "@phosphor-icons/react";

const indexText = (fields: Record<string, string>, slides: string[] = []) =>
  [
    "---",
    ...Object.entries(fields).map(([k, v]) => `${k}: ${v}`),
    "slides:",
    ...slides.map((s) => `  - ${s}`),
    "---",
    "# Lesson",
    "",
  ].join("\n");

const A1 = indexText(
  {
    ed_lesson_id: "123",
    title: '"Assignment 1"',
    lesson_type: '"python"',
    module: '"Assignments"',
    order: "4",
    state: '"active"',
    available_at: "null",
    due_at: '"2026-09-04T23:59:00+10:00"',
    locks_at: "null",
    url: '"https://example.org/courses/1/lessons/123"',
  },
  [
    '{"id":456,"title":"Q1: Group Project","type":"code","status":"completed","file":"123-assignment-1/01-q1-group-project.md","pdf":null,"url":"https://example.org/courses/1/lessons/123/slides/456","challenge_id":281410}',
    '{"id":457,"title":"Notes","type":"pdf","status":"unseen","file":"123-assignment-1/02-notes.md","pdf":"123-assignment-1/02-notes.pdf","url":null,"challenge_id":null}',
  ],
);

const file = (id: number, relative_path: string) =>
  ({ id, relative_path, filename: relative_path.slice(relative_path.lastIndexOf("/") + 1) }) as DbFile;

describe("Ed lesson index", () => {
  test("reads the front matter into an index", () => {
    const index = parseLessonIndex(A1)!;
    expect(index.lessonId).toBe(123);
    expect(index.title).toBe("Assignment 1");
    expect(index.lessonType).toBe("python");
    expect(index.module).toBe("Assignments");
    expect(index.order).toBe(4);
    expect(index.dueAt?.toISOString()).toBe("2026-09-04T13:59:00.000Z");
    expect(index.availableAt).toBeNull();
    expect(index.url).toBe("https://example.org/courses/1/lessons/123");
    expect(index.slides.map((s) => [s.id, s.type, s.status, s.pdf])).toEqual([
      [456, "code", "completed", null],
      [457, "pdf", "unseen", "123-assignment-1/02-notes.pdf"],
    ]);
    expect(lessonProgress(index)).toEqual({ done: 1, total: 2 });
    expect(index.slides.map((s) => s.challengeId)).toEqual([281410, null]);
  });

  test("reads a challenge slide's challenge and its snapshot", () => {
    const slide = (lines: string[]) =>
      ["---", "ed_slide_id: 456", ...lines, "---", "# Q1", ""].join("\n");
    const full = parseSlideChallenge(
      slide([
        "ed_challenge_id: 281410",
        'challenge: {"type":"pyunit","run_command":null,"features":{"run":true,"mark":true,"terminal":true,"feedback":true,"confirm_submit":false},"due_at":"2026-10-19T23:59:00+11:00"}',
      ]),
    )!;
    expect(full.challengeId).toBe(281410);
    expect(full.challenge?.type).toBe("pyunit");
    expect(full.challenge?.runCommand).toBeNull();
    expect(full.challenge?.features).toEqual({
      run: true,
      mark: true,
      terminal: true,
      feedback: true,
      confirmSubmit: false,
    });
    expect(full.challenge?.dueAt?.toISOString()).toBe("2026-10-19T12:59:00.000Z");

    // The settings could not be fetched: the id alone.
    expect(parseSlideChallenge(slide(["ed_challenge_id: 281410", "challenge: null"]))).toEqual({
      challengeId: 281410,
      challenge: null,
    });
    // Not a challenge slide, or a file from before the key existed.
    expect(parseSlideChallenge(slide(["ed_challenge_id: null", "challenge: null"]))).toBeNull();
    expect(parseSlideChallenge(slide([]))).toBeNull();
  });

  test("a lesson scraped before the split has no index", () => {
    expect(parseLessonIndex("# Week 1\n\n## Slide one\n\nBody")).toBeNull();
    const lesson = parseEdLesson("# Week 1", file(9, "courses/C/lessons/77-week-1.md"));
    expect(lesson.index).toBeNull();
    expect(lesson.lessonId).toBe(77);
    expect(lesson.title).toBe("Week 1");
  });

  test("groups by module in Ed's order, old lessons last and newest first", () => {
    const at = (id: number, module: string | null, order: number) =>
      parseEdLesson(
        indexText({ ed_lesson_id: String(id), title: `"L${id}"`, module: JSON.stringify(module), order: String(order) }),
        file(id, `courses/C/lessons/${id}-l.md`),
      );
    const old = (id: number) => parseEdLesson("# Old", file(id, `courses/C/lessons/${id}-old.md`));
    const groups = groupLessons([old(5), at(3, "Week 2", 3), at(1, "Week 1", 1), old(8), at(2, "Week 1", 0), at(4, null, 2)]);
    expect(groups.map((g) => g.label)).toEqual(["Week 1", "No module", "Week 2", "Other"]);
    expect(groups[0].lessons.map((l) => l.lessonId)).toEqual([2, 1]);
    expect(groups[3].key).toBe(OTHER_GROUP);
    expect(groups[3].lessons.map((l) => l.lessonId)).toEqual([8, 5]);
  });

  test("resolves a slide's files beside the index", () => {
    const index = file(1, "courses/C/lessons/123-assignment-1.md");
    const slides = parseLessonIndex(A1)!.slides;
    const md = file(2, "courses/C/lessons/123-assignment-1/02-notes.md");
    const pdf = file(3, "courses/C/lessons/123-assignment-1/02-notes.pdf");
    expect(slideFiles(index, slides[1], [index, md, pdf])).toEqual({ md, pdf });
    expect(slideFiles(index, slides[0], [index, md, pdf])).toEqual({ md: undefined, pdf: undefined });
    expect(resolveBeside("a/b/c.md", "./d/../e.md")).toBe("a/b/e.md");
  });

  test("opens on the first slide not yet completed", () => {
    const slides = parseLessonIndex(A1)!.slides;
    expect(defaultSlide(slides)?.id).toBe(457);
    expect(defaultSlide([{ ...slides[0] }])?.id).toBe(456);
    expect(defaultSlide([])).toBeNull();
  });

  test("maps slide and lesson types to glyphs", () => {
    expect(slideIcon("code")).toBe(Code);
    expect(slideIcon("something-new")).toBe(File);
    expect(lessonIcon("general")).toBe(FileText);
    expect(lessonIcon("python")).toBe(FilePy);
    expect(lessonIcon("rust")).toBe(FileCode);
  });
});
