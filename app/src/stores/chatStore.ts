import { create } from "zustand";
import { getChatMessages, type DbChatMessage, type ModelRef } from "@/lib/db";

export interface ToolActivity {
  name: string;
  /** Human-readable argument, e.g. the search query. */
  detail: string;
  done: boolean;
}

interface ChatState {
  /** null until the first message creates the chat in Rust. */
  chatId: number | null;
  messages: DbChatMessage[];
  /** Assistant text accumulating from `chat-delta` for the current turn. */
  streaming: string;
  /** Tool calls of the current turn, in order, for the activity line. */
  tools: ToolActivity[];
  sending: boolean;
  error: string | null;
  /** Model for the next turn — the composer's switcher, seeded from the
   *  configured chat model. Sent with each message, so switching mid-thread
   *  changes only what follows. */
  model: ModelRef | null;

  open: (chatId: number | null) => Promise<void>;
  setModel: (model: ModelRef) => void;
  begin: (chatId: number) => void;
  appendDelta: (delta: string) => void;
  toolEvent: (name: string, detail: string, status: string) => void;
  /** A committed row arrived; replace the live stream with it. */
  commit: (message: DbChatMessage) => void;
  finish: () => void;
  fail: (error: string) => void;
  reset: () => void;
}

export const useChatStore = create<ChatState>((set, get) => ({
  chatId: null,
  messages: [],
  streaming: "",
  tools: [],
  sending: false,
  error: null,
  model: null,

  setModel: (model) => set({ model }),

  open: async (chatId) => {
    if (chatId == null) {
      set({ chatId: null, messages: [], streaming: "", tools: [], error: null });
      return;
    }
    set({ chatId, streaming: "", tools: [], error: null });
    set({ messages: await getChatMessages(chatId) });
  },

  begin: (chatId) =>
    set({ chatId, sending: true, streaming: "", tools: [], error: null }),

  appendDelta: (delta) => set((s) => ({ streaming: s.streaming + delta })),

  toolEvent: (name, detail, status) =>
    set((s) => {
      if (status === "start") {
        return { tools: [...s.tools, { name, detail, done: false }] };
      }
      // Close the most recent open call of that name — calls of one round
      // arrive start-then-done in order.
      const tools = [...s.tools];
      for (let i = tools.length - 1; i >= 0; i--) {
        if (tools[i].name === name && !tools[i].done) {
          tools[i] = { ...tools[i], done: true };
          break;
        }
      }
      return { tools };
    }),

  commit: (message) =>
    set((s) => ({
      messages: [...s.messages, message],
      // The committed row carries the same text the stream was building.
      streaming: "",
    })),

  finish: () => set({ sending: false, tools: [] }),

  fail: (error) => set({ sending: false, error, streaming: "" }),

  reset: () => {
    const { chatId } = get();
    set({ chatId, messages: [], streaming: "", tools: [], sending: false, error: null });
  },
}));

/** Optimistic user row so the message appears before Rust has written it. */
export function localUserMessage(chatId: number | null, content: string): DbChatMessage {
  return {
    id: -Date.now(),
    chat_id: chatId ?? -1,
    role: "user",
    content,
    tool_calls: null,
    tool_call_id: null,
    citations: null,
    model: null,
    created_at: new Date().toISOString(),
  };
}
