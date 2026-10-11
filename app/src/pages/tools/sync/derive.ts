import {
  hasFailed,
  isComplete,
  statusOf,
  type PipelineItem,
} from "@/stores/sync/pipelineStore";

type EmbedStage = boolean;

/** How many rows sit in each phase of the file-activity table. */
export function countPhases(items: PipelineItem[], embedStage: EmbedStage) {
  let active = 0, waiting = 0, paused = 0, failed = 0, skipped = 0, done = 0;
  for (const it of items) {
    const phase = statusOf(it, embedStage).phase;
    if (phase === "active") active++;
    else if (phase === "waiting") waiting++;
    else if (phase === "paused") paused++;
    else if (phase === "failed") failed++;
    else if (phase === "skipped") skipped++;
    else done++;
  }
  return { active, waiting, paused, failed, skipped, done };
}

/** What "Clear finished" removes: completed and skipped rows, so a failure
 *  stays in view until it is retried or the file goes. */
export function countFinished(items: PipelineItem[], embedStage: EmbedStage): number {
  return items.filter((it) => isComplete(it, embedStage) && !hasFailed(it, embedStage)).length;
}
