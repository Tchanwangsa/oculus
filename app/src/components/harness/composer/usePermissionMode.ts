import { useState } from "react";
import { harnessSetPermissionMode, threadMode, type PermissionMode, type Provider } from "@/lib/harness";
import { useCapabilities } from "@/hooks/agents/useCapabilities";
import { useHarnessStore } from "@/stores/chat/harnessStore";
import { useNewChatMode } from "@/stores/chat/newChatMode";
import { useRunCommand } from "./useRunCommand";

/**
 * A chat's permission mode and the command that changes it, for every
 * control that shows it (`PermissionModeControl`, the compact composer's
 * settings menu). An open chat's pick is saved with
 * `harnessSetPermissionMode` from the next run on; Rust's refusal (a request
 * waiting) is `refused` until `clear`. Without a thread it is the mode the
 * next new chat starts with (`useNewChatMode`), sent with its first message.
 */
export function usePermissionMode(threadId: number | null, provider: Provider) {
  const caps = useCapabilities(provider);
  const stored = useHarnessStore((s) => (threadId == null ? undefined : s.threads.find((t) => t.id === threadId)?.permission_mode));
  const starting = useNewChatMode((s) => s.mode);
  // A chat outside the thread list (Studio's) hears no `permission_mode`
  // event, so a saved pick is kept here too.
  const [saved, setSaved] = useState<{ thread: number; mode: PermissionMode } | null>(null);
  const save = useRunCommand();
  const mode = threadId == null ? starting : threadMode(stored ?? (saved?.thread === threadId ? saved.mode : undefined));
  /** Save `next`; `done` runs once it is the chat's mode (at once when it already is). */
  const pick = (next: PermissionMode, done: () => void) => {
    if (threadId == null) {
      useNewChatMode.getState().set(next);
      return done();
    }
    if (next === mode) return done();
    void save
      .run(() => harnessSetPermissionMode(threadId, next))
      .then((ok) => {
        if (!ok) return;
        setSaved({ thread: threadId, mode: next });
        done();
      });
  };
  return { caps, mode, busy: save.pending, refused: save.error, pick, clear: save.clear };
}
