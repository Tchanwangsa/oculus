import { beforeEach, describe, expect, test } from "bun:test";
import { lessonProgress, type LessonIndex, type LessonSlide } from "@/lib/ed/lessons";
import { recordSlideStatus, useSlideStatusStore, withSlideStatuses } from "@/lib/ed/slideStatus";

const slide = (id: number | null, status: string | null): LessonSlide => ({
  id,
  title: `Slide ${id}`,
  type: "document",
  status,
  file: null,
  pdf: null,
  url: null,
  challengeId: null,
});

const index = (slides: LessonSlide[]): LessonIndex => ({
  lessonId: 1,
  title: "L",
  lessonType: null,
  module: null,
  order: null,
  dueAt: null,
  availableAt: null,
  locksAt: null,
  url: null,
  slides,
});

describe("slide statuses this session", () => {
  beforeEach(() => useSlideStatusStore.setState({ byId: new Map() }));

  test("raise the index's statuses and its count, and never lower one", () => {
    const lesson = index([slide(1, "unseen"), slide(2, "completed"), slide(3, "seen"), slide(null, "unseen")]);
    recordSlideStatus(1, "completed");
    recordSlideStatus(2, "seen");
    recordSlideStatus(3, "seen");
    const live = withSlideStatuses(lesson, useSlideStatusStore.getState().byId);
    expect(live.slides.map((s) => s.status)).toEqual(["completed", "completed", "seen", "unseen"]);
    expect(lessonProgress(live)).toEqual({ done: 2, total: 4 });

    recordSlideStatus(1, "seen");
    expect(useSlideStatusStore.getState().byId.get(1)).toBe("completed");
  });

  test("an index nothing raised comes back as itself", () => {
    const lesson = index([slide(1, "completed")]);
    recordSlideStatus(1, "seen");
    expect(withSlideStatuses(lesson, useSlideStatusStore.getState().byId)).toBe(lesson);
  });
});
