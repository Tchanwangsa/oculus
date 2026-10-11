import type { PipelineItem } from "@/stores/sync/pipelineStore";
import type { StageKey } from "@/components/sync/pipeline/constants";

/** An active embed held by a rate limit; its countdown ticks by the second. */
export function isRateLimited(item: PipelineItem): boolean {
  return item.embed === "active" && item.embedWaitingUntil != null;
}

/** Parse is in its batch but another file is uploading: not moving itself. */
export function uploadWaiting(item: PipelineItem): boolean {
  return item.parse === "active" && item.parsePhase === "upload_wait";
}

/** When the file last moved: the latest stage that finished, or was skipped. */
export function latestStageAt(item: PipelineItem): number {
  return Math.max(
    item.downloadedAt ?? 0,
    item.uploadedAt ?? 0,
    item.parsedAt ?? 0,
    item.embeddedAt ?? 0,
    item.skippedAt ?? 0,
  );
}

/** The stage whose failure the row's error belongs to. */
export function failedStage(item: PipelineItem): StageKey {
  if (item.download === "error") return "download";
  if (item.parse === "error") return "parse";
  return "embed";
}

export function stageKeys(embedStage: boolean): StageKey[] {
  return embedStage ? ["download", "parse", "embed"] : ["download", "parse"];
}
