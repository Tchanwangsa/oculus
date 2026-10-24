import { fsrs, type Card, type Grade } from "ts-fsrs";
import type { RevisionItem } from "./types";
const scheduler = fsrs({ enable_fuzz: false });
export function ratingForScore(score: number, max: number): Grade {
  const fraction = max > 0 ? score / max : 0;
  return fraction < 0.4 ? 1 : fraction < 0.7 ? 2 : fraction < 0.95 ? 3 : 4;
}
export function scheduledCard(
  item: RevisionItem,
  rating: Grade,
  now = new Date(),
) {
  const card: Card = {
    due: new Date(item.due),
    stability: item.stability,
    difficulty: item.fsrs_difficulty,
    elapsed_days: 0,
    scheduled_days: 0,
    learning_steps: item.learning_steps,
    reps: item.reps,
    lapses: item.lapses,
    state: item.state,
    last_review: item.last_review ? new Date(item.last_review) : undefined,
  };
  const next = scheduler.next(card, now, rating).card;
  return {
    due: next.due.toISOString(),
    stability: next.stability,
    fsrs_difficulty: next.difficulty,
    reps: next.reps,
    lapses: next.lapses,
    state: next.state,
    last_review: next.last_review?.toISOString() ?? null,
    learning_steps: next.learning_steps,
  };
}
export function localMark(
  item: Pick<RevisionItem, "kind" | "answer" | "tolerance">,
  answer: string,
): { score: number; max: number } | null {
  if (item.kind === "choice")
    return { score: Number(answer === item.answer), max: 1 };
  if (item.kind !== "numeric") return null;
  const value = answer.trim() === "" ? NaN : Number(answer);
  const expected = Number(item.answer);
  return {
    score: Number(
      Number.isFinite(value) &&
        Number.isFinite(expected) &&
        Math.abs(value - expected) <= (item.tolerance ?? 0),
    ),
    max: 1,
  };
}
