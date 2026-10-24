import { invoke } from "@tauri-apps/api/core";
import type { Grade } from "ts-fsrs";
import type { RevisionItem } from "./types";
import { scheduledCard } from "./scheduler";
import { notifyRevisionUpdated } from "./writes";
export interface ReviewInput {
  answer: string;
  rating: Grade;
  score?: number | null;
  max?: number | null;
  feedback?: string | null;
  judge_model?: string | null;
  overridden?: boolean;
  seconds?: number;
  set_id?: number | null;
  session_id?: string | null;
  scheduled?: boolean;
  working?: string | null;
}
export async function recordReview(item: RevisionItem, attempt: ReviewInput) {
  await invoke("revision_record_review", {
    input: {
      item_id: item.id,
      expected_reps: item.reps,
      ...attempt,
      scheduled: attempt.scheduled ?? true,
      overridden: attempt.overridden ?? false,
      card:
        attempt.scheduled === false
          ? null
          : scheduledCard(item, attempt.rating),
    },
  });
  notifyRevisionUpdated();
}
