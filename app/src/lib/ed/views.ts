import { invoke } from "@tauri-apps/api/core";
import { recordSlideStatus } from "./slideStatus";

const sent = new Set<string>();

/** Send `ed_mark_viewed` for `key` once per app session; a failure is logged
 *  and retried on the next open. */
function once(key: string, args: Record<string, unknown>, then?: (answer: string | null) => void): void {
  if (sent.has(key)) return;
  sent.add(key);
  invoke<string | null>("ed_mark_viewed", args).then(then, (e) => {
    sent.delete(key);
    console.warn(`Ed ${key} view not recorded:`, e);
  });
}

/** Tell Ed the reader opened thread `id`; see docs/sync.md. */
export function markEdViewed(kind: "thread", id: number): void {
  once(`${kind}:${id}`, { kind, id });
}

/** Tell Ed the reader opened slide `id` of Ed type `type`, now `status`:
 *  Rust marks it seen and completes a reading slide, as Ed's web app does
 *  (docs/sync.md). The status Ed then holds shows at once
 *  (`recordSlideStatus`). */
export function markSlideOpened(id: number, type: string | null, status: string | null): void {
  once(`slide:${id}`, { kind: "slide", id, slideType: type, status }, (answer) => {
    if (answer) recordSlideStatus(id, answer);
  });
}

/** The Lessons tab reads every slide's status from Ed this often at most. */
const REFRESH_MS = 2 * 60 * 1000;
const refreshedAt = new Map<string, number>();

/** Ask Ed for the status of every slide of Ed lessons `lessonIds` and record
 *  what it holds (`recordSlideStatus`), so the Lessons tab's counts are
 *  Ed's now and not the last sync's. A read, not a view; a failure is
 *  logged and the next open tries again. */
export function refreshSlideStatuses(lessonIds: number[]): void {
  if (lessonIds.length === 0) return;
  const key = [...lessonIds].sort((a, b) => a - b).join(",");
  const last = refreshedAt.get(key);
  if (last != null && Date.now() - last < REFRESH_MS) return;
  refreshedAt.set(key, Date.now());
  invoke<{ id: number; status: string }[]>("ed_slide_statuses", { lessonIds }).then(
    (all) => all.forEach(({ id, status }) => recordSlideStatus(id, status)),
    (e) => {
      refreshedAt.delete(key);
      console.warn("Ed slide statuses not refreshed:", e);
    },
  );
}
