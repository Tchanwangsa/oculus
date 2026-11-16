import { invoke } from "@tauri-apps/api/core";
import { commandId } from "./commands";

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
