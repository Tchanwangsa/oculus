import { invoke } from "@tauri-apps/api/core";
import type { Provider } from "./providers";
import type { ChatScope } from "./scope";
import type { RunRow } from "./runs";
import type { BridgeHealth, HarnessItem, QueuedMessage } from "./types";
import type { StudioSend } from "@/lib/studio/videos";
import { openNotebookContext } from "@/lib/canvas/openNotebooks";
import { useProposalStore } from "@/stores/chat/proposalStore";

export interface SendOptions {
  model?: string | null;
  reasoningEffort?: string | null;
  /** Only read when the send creates a Studio thread (`studio`). */
  subjectId?: number | null;
  /** Only read when the send creates a plain Chat thread, which Rust refuses
   *  to create without one (`docs/harness.md`, "Subject scope and `@`"). */
  chatScope?: ChatScope;
  /** Only read on creation; overrides `subjectId`. */
  lectureId?: string | null;
  /** What the student has open (a lecture's moment, a notebook), appended to the prompt but never to the message. */
  context?: string | null;
  /** Playhead second; stored on the user row's `meta` as `{ at }`. */
  at?: number | null;
  /** Only read on creation: links the new thread to a Studio video. */
  studio?: StudioSend;
}

type MessagingInvoke = <T>(command: string, args: Record<string, unknown>) => Promise<T>;

/** An id for one thing the student did. Rust keeps the result under it
 *  (`harness_receipts`), so the same id sent again changes nothing and
 *  answers as the first call did. */
export function commandId(): string {
  return crypto.randomUUID();
}

/** Keep prompt dispatch behind card saves for every entry point. Explicit
 * dependencies let this ordering be checked without mocking global IPC. */
export function createHarnessMessaging(invokeCommand: MessagingInvoke, flushThread: (thread: number) => Promise<void>) {
  return {
    send: async (threadId: number | null, provider: Provider, text: string, options: SendOptions): Promise<number> => {
      if (threadId != null) await flushThread(threadId);
      return invokeCommand<number>("harness_send", { commandId: commandId(), threadId, provider, text, options });
    },
    editResend: async (threadId: number, itemId: number, text: string, options: SendOptions): Promise<void> => {
      await flushThread(threadId);
      return invokeCommand("harness_edit_resend", { commandId: commandId(), threadId, itemId, text, options });
    },
  };
}

const messaging = createHarnessMessaging(invoke, (thread) => useProposalStore.getState().flushThread(thread));

export function harnessSend(
  threadId: number | null,
  provider: Provider,
  text: string,
  options: SendOptions,
): Promise<number> {
  const notebooks = openNotebookContext();
  const context = [options.context, notebooks].filter(Boolean).join("\n\n") || null;
  return messaging.send(threadId, provider, text, notebooks ? { ...options, context } : options);
}

/** Rewind to a question and send new text in its place. See docs/harness.md
 *  "Going back" for when the agent's own context cannot follow. */
export const harnessEditResend = messaging.editResend;

/** `harnessEditResend` without the send: resolves to the question's text. */
export function harnessRewind(threadId: number, itemId: number): Promise<string> {
  return invoke<string>("harness_rewind", { commandId: commandId(), threadId, itemId });
}

/** A thread as Rust read it in one transaction (`harness/core/load.rs`):
 *  its rows and runs, the revision that covers them, and what waits behind
 *  its turn. An envelope at or below `revision` tells of nothing new. */
export interface HarnessSnapshot {
  revision: number;
  items: HarnessItem[];
  runs: RunRow[];
  queued: QueuedMessage[];
}

export function harnessLoad(threadId: number): Promise<HarnessSnapshot> {
  return invoke<HarnessSnapshot>("harness_load", { threadId });
}

export function harnessUnqueue(threadId: number, queueId: string): Promise<void> {
  return invoke("harness_unqueue", { commandId: commandId(), threadId, queueId });
}

export function harnessEditQueued(threadId: number, queueId: string, text: string): Promise<void> {
  return invoke("harness_edit_queued", { commandId: commandId(), threadId, queueId, text });
}

/** Put the waiting messages in `order` (their ids), first to go out first. */
export function harnessReorderQueued(threadId: number, order: string[]): Promise<void> {
  return invoke("harness_reorder_queued", { commandId: commandId(), threadId, order });
}

/** Stop a run and hold what waits behind it. `runId` is the run on screen
 *  (`liveRunId`); null stops whatever turn is open, one the agent started
 *  itself included. Stop for a run that already finished touches nothing.
 *  The turn ends when its CLI says so (`turn_finished`). */
export function harnessInterrupt(threadId: number, runId: number | null): Promise<void> {
  return invoke("harness_interrupt", { commandId: commandId(), threadId, runId });
}

/** Lift the hold on a thread's queue (after a stop, a failed turn or a
 *  restart): its waiting messages go out again, in order. */
export function harnessResume(threadId: number): Promise<void> {
  return invoke("harness_resume", { commandId: commandId(), threadId });
}

export function harnessDeleteThread(threadId: number): Promise<void> {
  return invoke("harness_delete_thread", { commandId: commandId(), threadId });
}

/** The thread is on screen: stamps `viewed_at` (never `updated_at`) and
 *  returns it; null when the thread is gone. */
export async function harnessThreadViewed(threadId: number): Promise<string | null> {
  return invoke("harness_thread_viewed", { threadId });
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
