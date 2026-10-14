import {
  ChalkboardTeacher,
  Code,
  File,
  FileCode,
  FilePdf,
  FilePy,
  FileText,
  Globe,
  ListChecks,
  Notebook,
  VideoCamera,
  type Icon,
} from "@phosphor-icons/react";
import type { DbFile } from "@/lib/db";
import { humanizeSlug } from "@/lib/format/format";
import { frontNumber, frontString, splitFrontmatter, type FrontValue } from "@/lib/markdown/frontmatter";

/** A lesson's own markdown, `courses/<CODE>/lessons/<id>-<slug>.md`. Its
 *  slides, PDFs and challenge files sit one folder down. */
const LESSON_DOC = /^courses\/[^/]+\/lessons\/[^/]+\.md$/;

export const isLessonDoc = (file: Pick<DbFile, "relative_path">): boolean =>
  LESSON_DOC.test(file.relative_path);

/** One slide as the lesson index lists it. `file` and `pdf` are relative to
 *  the index's folder; `status` is Ed's `unseen` | `seen` | `completed`;
 *  `challengeId` is set on a `code` or `jupyter` slide, whose workspace it is. */
export interface LessonSlide {
  id: number | null;
  title: string;
  type: string | null;
  status: string | null;
  file: string | null;
  pdf: string | null;
  url: string | null;
  challengeId: number | null;
}

/** What a challenge slide's file says about its workspace (docs/sync.md).
 *  A feature Ed did not name is null, not off; `runCommand` null means Ed
 *  has none, so Run does nothing. */
export interface ChallengeSnapshot {
  type: string | null;
  runCommand: string | null;
  features: {
    run: boolean | null;
    mark: boolean | null;
    terminal: boolean | null;
    feedback: boolean | null;
    confirmSubmit: boolean | null;
  };
  dueAt: Date | null;
}

/** A slide file's challenge: its id, and the snapshot when the sync got one. */
export interface SlideChallenge {
  challengeId: number;
  challenge: ChallengeSnapshot | null;
}

/** A lesson index's front matter (the contract is in docs/sync.md). */
export interface LessonIndex {
  lessonId: number | null;
  title: string | null;
  lessonType: string | null;
  module: string | null;
  order: number | null;
  dueAt: Date | null;
  availableAt: Date | null;
  locksAt: Date | null;
  url: string | null;
  slides: LessonSlide[];
}

const frontDate = (value: FrontValue | undefined): Date | null => {
  const text = frontString(value);
  if (!text) return null;
  const date = new Date(text);
  return Number.isNaN(date.getTime()) ? null : date;
};

function parseSlide(value: FrontValue, index: number): LessonSlide | null {
  if (!value || typeof value !== "object" || Array.isArray(value)) return null;
  return {
    id: frontNumber(value.id),
    title: frontString(value.title) ?? `Slide ${index + 1}`,
    type: frontString(value.type),
    status: frontString(value.status),
    file: frontString(value.file),
    pdf: frontString(value.pdf),
    url: frontString(value.url),
    challengeId: frontNumber(value.challenge_id),
  };
}

const frontBool = (value: FrontValue | undefined): boolean | null =>
  typeof value === "boolean" ? value : null;

const isObject = (value: FrontValue | undefined): value is { [key: string]: FrontValue } =>
  value != null && typeof value === "object" && !Array.isArray(value);

/** The challenge in a slide file's front matter, or null for a slide that
 *  is not a challenge's. */
export function parseSlideChallenge(markdown: string): SlideChallenge | null {
  const { data } = splitFrontmatter(markdown);
  const challengeId = frontNumber(data.ed_challenge_id);
  if (challengeId == null) return null;
  const snapshot = data.challenge;
  if (!isObject(snapshot)) return { challengeId, challenge: null };
  const features = isObject(snapshot.features) ? snapshot.features : {};
  return {
    challengeId,
    challenge: {
      type: frontString(snapshot.type),
      runCommand: frontString(snapshot.run_command),
      features: {
        run: frontBool(features.run),
        mark: frontBool(features.mark),
        terminal: frontBool(features.terminal),
        feedback: frontBool(features.feedback),
        confirmSubmit: frontBool(features.confirm_submit),
      },
      dueAt: frontDate(snapshot.due_at),
    },
  };
}

/** The index in `markdown`'s front matter, or null for a lesson file
 *  without one (every slide's content in the one file), which stays plain
 *  markdown. */
export function parseLessonIndex(markdown: string): LessonIndex | null {
  const { data } = splitFrontmatter(markdown);
  if (!("ed_lesson_id" in data) && !("slides" in data)) return null;
  const slides = Array.isArray(data.slides)
    ? data.slides.map(parseSlide).filter((s): s is LessonSlide => s != null)
    : [];
  return {
    lessonId: frontNumber(data.ed_lesson_id),
    title: frontString(data.title),
    lessonType: frontString(data.lesson_type),
    module: frontString(data.module),
    order: frontNumber(data.order),
    dueAt: frontDate(data.due_at),
    availableAt: frontDate(data.available_at),
    locksAt: frontDate(data.locks_at),
    url: frontString(data.url),
    slides,
  };
}

/** A lesson row: its file, its index when it has one, and what the list shows. */
export interface EdLesson {
  file: DbFile;
  index: LessonIndex | null;
  title: string;
  lessonId: number;
}

/** `useCourseFileData`'s parser for the Lessons tab. */
export function parseEdLesson(markdown: string, file: DbFile): EdLesson {
  const index = parseLessonIndex(markdown);
  return {
    file,
    index,
    title: index?.title ?? humanizeSlug(file.filename),
    lessonId: index?.lessonId ?? Number(/^(\d+)-/.exec(file.filename)?.[1] ?? 0),
  };
}

/** Slides completed and slides in all, from the statuses. */
export function lessonProgress(index: LessonIndex): { done: number; total: number } {
  return {
    done: index.slides.filter((s) => s.status === "completed").length,
    total: index.slides.length,
  };
}

export interface LessonGroup {
  /** Ed's module name, or a sentinel no module can take: lessons outside a
   *  module, and `OTHER_GROUP`. */
  key: string;
  label: string;
  lessons: EdLesson[];
}

/** The group key of lesson files without an index, which carry no module
 *  or order. */
export const OTHER_GROUP = "\u0000other";
const NO_MODULE = "\u0000none";

/**
 * Lessons under their Ed module, modules in the order their first lesson
 * takes in Ed's ordering, lessons by `order` within each. Lessons without an
 * index fall into a last "Other" group, newest (highest Ed id) first.
 */
export function groupLessons(lessons: readonly EdLesson[]): LessonGroup[] {
  const byKey = new Map<string, EdLesson[]>();
  const other: EdLesson[] = [];
  for (const lesson of lessons) {
    if (!lesson.index) {
      other.push(lesson);
      continue;
    }
    const key = lesson.index.module ?? NO_MODULE;
    byKey.set(key, [...(byKey.get(key) ?? []), lesson]);
  }
  const rank = (l: EdLesson) => l.index?.order ?? Number.MAX_SAFE_INTEGER;
  const byOrder = (a: EdLesson, b: EdLesson) => rank(a) - rank(b) || a.lessonId - b.lessonId;
  const groups: LessonGroup[] = [...byKey].map(([key, members]) => ({
    key,
    label: key === NO_MODULE ? "No module" : key,
    lessons: members.sort(byOrder),
  }));
  groups.sort((a, b) => byOrder(a.lessons[0], b.lessons[0]));
  if (other.length > 0) {
    // Ed's lesson ids rise as lessons are made; not zero-padded, so numeric.
    other.sort((a, b) => b.lessonId - a.lessonId || a.file.filename.localeCompare(b.file.filename));
    groups.push({ key: OTHER_GROUP, label: "Other", lessons: other });
  }
  return groups;
}

/** A path relative to `fromFile`'s folder, as a library-relative path. */
export function resolveBeside(fromFile: string, relative: string): string {
  const parts = fromFile.split("/").slice(0, -1);
  for (const segment of relative.split("/")) {
    if (segment === "" || segment === ".") continue;
    if (segment === "..") parts.pop();
    else parts.push(segment);
  }
  return parts.join("/");
}

/** The slide's file rows: its `.md` and, for a PDF slide, the PDF. Either is
 *  undefined until a sync has stored it. */
export function slideFiles(
  index: Pick<DbFile, "relative_path">,
  slide: LessonSlide,
  files: readonly DbFile[],
): { md: DbFile | undefined; pdf: DbFile | undefined } {
  const find = (rel: string | null) => {
    if (!rel) return undefined;
    const path = resolveBeside(index.relative_path, rel);
    return files.find((f) => f.relative_path === path);
  };
  return { md: find(slide.file), pdf: find(slide.pdf) };
}

/** Where a lesson view opens: the first slide not yet completed, else the
 *  first. Null for a lesson without slides. */
export function defaultSlide(slides: readonly LessonSlide[]): LessonSlide | null {
  return slides.find((s) => s.status !== "completed") ?? slides[0] ?? null;
}

/** A slide's key in the URL (`&slide=`): Ed's id, else its 1-based position. */
export const slideKey = (slide: LessonSlide, position: number): string =>
  slide.id != null ? String(slide.id) : `n${position + 1}`;

/** Ed's slide type, as a glyph. */
export function slideIcon(type: string | null): Icon {
  switch (type) {
    case "document": return FileText;
    case "code": return Code;
    case "jupyter": return Notebook;
    case "pdf": return FilePdf;
    case "quiz": return ListChecks;
    case "video": return VideoCamera;
    case "html":
    case "webpage": return Globe;
    default: return File;
  }
}

/** Ed's lesson type, as a glyph: a reading for `general`, code for a
 *  language lesson, the lesson glyph when there is no index. */
export function lessonIcon(type: string | null): Icon {
  if (type == null) return ChalkboardTeacher;
  if (type === "general") return FileText;
  if (type === "python") return FilePy;
  return FileCode;
}
