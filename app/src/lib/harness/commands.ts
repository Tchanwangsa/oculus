import { invoke } from "@tauri-apps/api/core";
import type { Provider } from "./providers";
import type { BridgeHealth, QueuedMessage } from "./types";

export interface SendOptions {
  model?: string | null;
  reasoningEffort?: string | null;
  /** Only read when the send creates the thread. */
  subjectId?: number | null;
  /** Only read on creation; overrides `subjectId`. */
  lectureId?: string | null;
  /** The lecture moment, appended to the prompt but never to the message. */
  context?: string | null;
  /** Playhead second; stored on the user row's `meta` as `{ at }`. */
  at?: number | null;
}

export function harnessSend(
  threadId: number | null,
  provider: Provider,
  text: string,
  options: SendOptions,
): Promise<number> {
  return invoke<number>("harness_send", { threadId, provider, text, options });
}

/** Rewind to a question and send new text in its place. See docs/harness.md
 *  "Going back" for when the agent's own context cannot follow. */
export function harnessEditResend(
  threadId: number,
  itemId: number,
  text: string,
  options: SendOptions,
): Promise<void> {
  return invoke("harness_edit_resend", { threadId, itemId, text, options });
}

/** `harnessEditResend` without the send: resolves to the question's text. */
export function harnessRewind(threadId: number, itemId: number): Promise<string> {
  return invoke<string>("harness_rewind", { threadId, itemId });
}

export function harnessQueued(threadId: number): Promise<QueuedMessage[]> {
  return invoke<QueuedMessage[]>("harness_queued", { threadId });
}

export function harnessUnqueue(threadId: number, queueId: string): Promise<void> {
  return invoke("harness_unqueue", { threadId, queueId });
}

export function harnessEditQueued(threadId: number, queueId: string, text: string): Promise<void> {
  return invoke("harness_edit_queued", { threadId, queueId, text });
}

/** Stop the turn and clear the queue; resolves to the dropped messages. */
export function harnessInterrupt(threadId: number): Promise<string[]> {
  return invoke<string[]>("harness_interrupt", { threadId });
}

export function harnessDeleteThread(threadId: number): Promise<void> {
  return invoke("harness_delete_thread", { threadId });
}

/** Read through `useBridgeHealth`. `recheck` drops Rust's cached lookups
 *  (a login shell per provider) — Settings' Recheck button only. */
export function harnessHealth(recheck = false): Promise<BridgeHealth[]> {
  return invoke<BridgeHealth[]>("harness_health", { recheck });
}

/** How a CLI was installed, read from its binary's real path in Rust. */
export type UpdateSource = "brew" | "npm" | "bun" | "selfManaged";

/** One installed CLI against its newest published version. `available` is
 *  never true when the check failed (`error`). */
export interface UpdateInfo {
  provider: Provider;
  installed: string | null;
  latest: string | null;
  available: boolean;
  source: UpdateSource;
  /** The literal command `harnessUpdateRun` runs, shown with Copy. */
  command: string;
  error: string | null;
}

/** Missing CLIs are left out. Rust caches the registry answers for hours;
 *  `recheck` drops them. */
export function harnessUpdates(recheck = false): Promise<UpdateInfo[]> {
  return invoke<UpdateInfo[]>("harness_updates", { recheck });
}

/** Output streams on `harness-update`; one update runs at a time, so a second
 *  call while one runs rejects. */
export function harnessUpdateRun(provider: Provider): Promise<void> {
  return invoke<void>("harness_update_run", { provider });
}

/** The answer arrives as a `rate_limits` event. Claude ignores it. */
export function harnessRefreshRateLimits(provider: Provider): Promise<void> {
  return invoke<void>("harness_refresh_rate_limits", { provider });
}
