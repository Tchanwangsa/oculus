import type { PipelineItem } from "./item";

/** Below this much sampling, an upload's rate is noise. */
const ETA_MIN_SAMPLE_MS = 10_000;

/** Milliseconds an upload has left at its latest sample, or null until
 *  `ETA_MIN_SAMPLE_MS` of samples show it moving. */
export function uploadEta(it: PipelineItem): number | null {
  if (it.parse !== "active" || it.parsePhase !== "uploading") return null;
  const { uploadFirstAt: t0, uploadSampleAt: t1, bytesDone, bytesTotal } = it;
  if (t0 == null || t1 == null || bytesDone == null || !bytesTotal) return null;
  const span = t1 - t0;
  const moved = bytesDone - (it.uploadFirstBytes ?? 0);
  if (span < ETA_MIN_SAMPLE_MS || moved <= 0) return null;
  return Math.max(0, bytesTotal - bytesDone) / (moved / span);
}

/** "~2 h left", "~1.5 h left", "~40 min left", "~3 min left", "under a
 *  minute left": coarser as the wait grows, since the rate wobbles. */
export function fmtEta(ms: number): string {
  const mins = ms / 60_000;
  if (mins < 1) return "under a minute left";
  if (mins < 10) return `~${Math.round(mins)} min left`;
  if (mins < 55) return `~${Math.round(mins / 5) * 5} min left`;
  const hours = mins / 60;
  if (hours < 10) {
    const h = Math.max(1, Math.round(hours * 2) / 2);
    return `~${h} h left`;
  }
  return `~${Math.round(hours)} h left`;
}
