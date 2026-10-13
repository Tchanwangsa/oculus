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
