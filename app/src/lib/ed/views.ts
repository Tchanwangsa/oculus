import { invoke } from "@tauri-apps/api/core";

const sent = new Set<string>();

/** Tell Ed the reader opened thread or slide `id`, once per app session; see
 *  docs/sync.md. A failure is logged and retried on the next open. */
export function markEdViewed(kind: "thread" | "slide", id: number): void {
  const key = `${kind}:${id}`;
  if (sent.has(key)) return;
  sent.add(key);
  invoke("ed_mark_viewed", { kind, id }).catch((e) => {
    sent.delete(key);
    console.warn(`Ed ${kind} ${id} view not recorded:`, e);
  });
}
