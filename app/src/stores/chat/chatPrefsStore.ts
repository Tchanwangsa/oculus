import { useEffect } from "react";
import { create } from "zustand";
import { getSetting, setSetting } from "@/lib/db";
import { DEFAULT_FOLLOW_UP, type FollowUp } from "@/lib/harness/sendMode";

/** Chat preferences, in SQLite's `settings` table: what Enter does while the
 *  agent works (Settings → Chat). Until the row is read the default holds,
 *  so the composer never waits on it. */

const FOLLOW_UP_KEY = "chat_follow_up";

interface ChatPrefs {
  followUp: FollowUp;
  loaded: boolean;
  load: () => Promise<void>;
  setFollowUp: (next: FollowUp) => Promise<void>;
}

export const parseFollowUp = (raw: string | null): FollowUp => (raw === "queue" ? "queue" : DEFAULT_FOLLOW_UP);

let loading: Promise<void> | null = null;

export const useChatPrefsStore = create<ChatPrefs>((set, get) => ({
  followUp: DEFAULT_FOLLOW_UP,
  loaded: false,
  load: () => {
    if (get().loaded) return Promise.resolve();
    loading ??= getSetting(FOLLOW_UP_KEY)
      .then((raw) => set({ followUp: parseFollowUp(raw), loaded: true }))
      // A failed read keeps the default and is asked again by the next reader.
      .finally(() => {
        loading = null;
      });
    return loading;
  },
  setFollowUp: async (next) => {
    set({ followUp: next, loaded: true });
    await setSetting(FOLLOW_UP_KEY, next);
  },
}));

/** The chat's follow-up behaviour, read once from SQLite on first use. */
export function useFollowUp(): FollowUp {
  const { followUp, loaded, load } = useChatPrefsStore();
  useEffect(() => {
    if (!loaded) void load().catch(() => {});
  }, [loaded, load]);
  return followUp;
}
