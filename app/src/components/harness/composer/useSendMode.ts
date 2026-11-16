import { useEffect, useState } from "react";
import { useSteerKind } from "@/hooks/agents/useSteerKind";
import { isMac } from "@/lib/platform";
import { deliveryMode, harnessSteerQueued, isSlashCommand, sendAction, type DeliveryMode, type SendAction } from "@/lib/harness";
import { useFollowUp } from "@/stores/chat/chatPrefsStore";
import { useHarnessStore } from "@/stores/chat/harnessStore";
import { useRunCommand } from "./useRunCommand";

/** Whether ⌘ (Ctrl off macOS) is held, listened for only while `on`: the
 *  send button flips with it, so it shows what a ⌘-click or ⌘↩ will do. */
function useModHeld(on: boolean): boolean {
  const [held, setHeld] = useState(false);
  useEffect(() => {
    if (!on) {
      setHeld(false);
      return;
    }
    const key = isMac ? "Meta" : "Control";
    const read = (e: KeyboardEvent) => setHeld(isMac ? e.metaKey : e.ctrlKey);
    const up = (e: KeyboardEvent) => {
      if (e.key === key) setHeld(false);
      else read(e);
    };
    // A ⌘-Tab away never sends the keyup.
    const reset = () => setHeld(false);
    window.addEventListener("keydown", read);
    window.addEventListener("keyup", up);
    window.addEventListener("blur", reset);
    return () => {
      window.removeEventListener("keydown", read);
      window.removeEventListener("keyup", up);
      window.removeEventListener("blur", reset);
    };
  }, [on]);
  return held;
}

/**
 * The composer's send mode for `threadId` while its agent works: what the
 * draft `text` would do now (`shown`, flipped live while ⌘ is held), the
 * mode a send carries (`modeFor`, with or without the flip), and ⌘⇧↩'s
 * steer of the oldest waiting message, whose refusal is `error`.
 */
export function useSendMode({ threadId, running, text }: { threadId: number | null; running: boolean; text: string }) {
  const steer = useSteerKind(threadId);
  const followUp = useFollowUp();
  const flipped = useModHeld(running && steer != null);
  const command = isSlashCommand(text);
  const actionFor = (flip: boolean): SendAction => sendAction({ running, steer, followUp, flip, command });
  const oldest = useHarnessStore((s) => (threadId == null ? null : (s.queued[threadId]?.[0]?.id ?? null)));
  const steerOldest = useRunCommand();
  const { clear } = steerOldest;
  useEffect(() => clear(), [threadId, running, clear]);
  return {
    steer,
    shown: actionFor(flipped),
    /** What Enter does with an empty box's next message: the hint's subject. */
    plain: sendAction({ running, steer, followUp, flip: false, command: false }),
    modeFor: (flip: boolean): DeliveryMode | undefined => deliveryMode(actionFor(flip)),
    /** ⌘⇧↩: the oldest waiting message into the turn, where it can go. */
    steerOldest: () => {
      if (threadId == null || oldest == null || steer == null) return;
      void steerOldest.run(() => harnessSteerQueued(threadId, oldest));
    },
    error: steerOldest.error ? `Couldn't send it now: ${steerOldest.error}` : null,
  };
}
