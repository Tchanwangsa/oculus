import { invoke } from "@tauri-apps/api/core";
import { commandId } from "./commands";
import type { HarnessThread } from "./types";

/** Payload: changed thread ids (number[]), emitted after commands and sweeps.
 *  Refresh open/completed reads when this event arrives. */
export const HARNESS_THREADS_CHANGED_EVENT = "harness-threads-changed";
export const MENU_COMPLETE_CHAT_EVENT = "menu-complete-chat";

export function harnessCompleteThread(threadId: number): Promise<void> {
  return invoke("harness_complete_thread", { commandId: commandId(), threadId });
}

export function harnessReopenThread(threadId: number): Promise<void> {
  return invoke("harness_reopen_thread", { commandId: commandId(), threadId });
}

/** The store's list without the completed threads a view holds on screen. */
export const openThreads = (threads: HarnessThread[]): HarnessThread[] => threads.filter((t) => !t.completed_at);

/** The thread ⌘E completes: the open chat, only in the focused Chat pane
 *  (the menu item is always enabled), and only while it is still open. */
export function completeChatTarget(focused: boolean, thread: HarnessThread | null): number | null {
  return focused && thread && !thread.completed_at ? thread.id : null;
}
