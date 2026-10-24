import { describe, expect, test } from "bun:test";
import {
  localMark,
  ratingForScore,
  scheduledCard,
} from "@/lib/revision/scheduler";
import type { RevisionItem } from "@/lib/revision/types";
describe("Revision grading and scheduling", () => {
  test("score boundaries select the documented ratings", () => {
    for (const [score, rating] of [
      [0, 1],
      [39, 1],
      [40, 2],
      [69, 2],
      [70, 3],
      [94, 3],
      [95, 4],
      [100, 4],
    ])
      expect(ratingForScore(score, 100)).toBe(rating);
  });
  test("numeric marks reject blank and nonfinite values and respect tolerance", () => {
    const item = { kind: "numeric" as const, answer: "4", tolerance: 0.1 };
    expect(localMark(item, "4.09")?.score).toBe(1);
    for (const answer of ["", "Infinity", "NaN", "4.2"])
      expect(localMark(item, answer)?.score).toBe(0);
  });
  test("a new card schedules learning and a recall schedules a later review", () => {
    const now = new Date("2026-10-24T00:00:00Z");
    const item = {
      due: now.toISOString(),
      stability: 0,
      fsrs_difficulty: 0,
      reps: 0,
      lapses: 0,
      state: 0,
      learning_steps: 0,
      last_review: null,
    } as RevisionItem;
    const again = scheduledCard(item, 1, now);
    const easy = scheduledCard(item, 4, now);
    expect(again.reps).toBe(1);
    expect(easy.reps).toBe(1);
    expect(new Date(again.due).getTime()).toBeGreaterThan(now.getTime());
    expect(new Date(easy.due).getTime()).toBeGreaterThan(
      new Date(again.due).getTime(),
    );
    expect(easy.last_review).toBe(now.toISOString());
  });
});

test("choice and numeric question validation prevents unmarkable items", async () => {
  const { validateItem } = await import("@/lib/revision/validation");
  expect(() =>
    validateItem({
      kind: "choice",
      prompt: "Pick",
      answer: "C",
      options: ["A", "B"],
    }),
  ).toThrow();
  expect(() =>
    validateItem({ kind: "numeric", prompt: "Count", answer: "Infinity" }),
  ).toThrow();
  expect(() =>
    validateItem({
      kind: "choice",
      prompt: "Pick",
      answer: "A",
      options: ["A", "B"],
    }),
  ).not.toThrow();
});

test("choice grading preserves exact option text",()=>{expect(localMark({kind:"choice",answer:"A",tolerance:null}," A ")?.score).toBe(0);});
