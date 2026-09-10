import { useState, useRef, useEffect, useMemo } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import remarkMath from "remark-math";
import rehypeKatex from "rehype-katex";
import {
  CircleNotch,
  FileText,
  MagnifyingGlass,
  PaperPlaneTilt,
  Plus,
  Stop,
} from "@phosphor-icons/react";
import { Button } from "@/components/ui/button";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Textarea } from "@/components/ui/textarea";
import { MD_COMPONENTS } from "@/components/markdown/MdComponents";
import { ModelSelect } from "@/components/llm/ModelSelect";
import {
  getChats,
  getLlmSettings,
  type DbChat,
  type DbChatMessage,
  type LlmSettings,
} from "@/lib/db";
import { useChatStore, localUserMessage } from "@/stores/chatStore";
import { cn } from "@/lib/utils";

const SUGGESTIONS = [
  "What's due this week?",
  "Summarise this week's lecture slides",
  "Explain the Bloch sphere",
  "Find the worked example on Dijkstra",
];

const TOOL_LABEL: Record<string, string> = {
  search_library: "Searching the library",
  read_file: "Reading",
  list_subjects: "Listing subjects",
  list_files: "Listing files",
};

interface Citation {
  subject_id: number;
  relative_path: string;
  filename: string;
  page_no: number | null;
}

/** The agent cites with `oculus-file://<subjectId>/<relativePath>?page=N`;
 *  those open the local copy instead of the browser. */
function openCitation(href: string) {
  const path = href.replace(/^oculus-file:\/\/\d+\//, "").split("?")[0];
  invoke("open_course_file", { relativePath: decodeURI(path) }).catch(() => {});
}

const CHAT_MD = {
  ...MD_COMPONENTS,
  a: ({ href, children, ...p }: any) =>
    href?.startsWith("oculus-file://") ? (
      <button
        type="button"
        onClick={() => openCitation(href)}
        className="text-brand hover:underline inline"
        {...p}
      >
        {children}
      </button>
    ) : (
      (MD_COMPONENTS.a as any)({ href, children, ...p })
    ),
};

function Message({ m }: { m: DbChatMessage }) {
  const citations = useMemo<Citation[]>(() => {
    if (!m.citations) return [];
    try {
      return JSON.parse(m.citations);
    } catch {
      return [];
    }
  }, [m.citations]);

  // One chip per document. Keyed on filename, not path: Canvas serves the
  // same file from both a module folder and the files list, so a path-keyed
  // dedupe still shows the same name twice.
  const unique = useMemo(() => {
    const seen = new Map<string, Citation>();
    for (const c of citations) if (!seen.has(c.filename)) seen.set(c.filename, c);
    return [...seen.values()];
  }, [citations]);

  if (m.role === "user") {
    return (
      <div className="flex justify-end">
        <div className="max-w-[85%] rounded-xl bg-surface-raised px-3.5 py-2 text-sm text-foreground whitespace-pre-wrap">
          {m.content}
        </div>
      </div>
    );
  }

  return (
    <div className="max-w-none">
      <ReactMarkdown
        remarkPlugins={[remarkGfm, remarkMath]}
        rehypePlugins={[rehypeKatex]}
        components={CHAT_MD}
      >
        {m.content ?? ""}
      </ReactMarkdown>

      {unique.length > 0 && (
        <div className="mt-3 flex flex-wrap gap-1.5">
          {unique.map((c) => (
            <button
              key={c.relative_path}
              type="button"
              onClick={() =>
                invoke("open_course_file", { relativePath: c.relative_path }).catch(() => {})
              }
              className="inline-flex items-center gap-1.5 rounded-md border border-border bg-surface px-2 py-1 text-[11px] text-muted-foreground hover:text-foreground hover:bg-accent transition-colors"
            >
              <FileText size={11} className="shrink-0" />
              <span className="truncate max-w-[220px]">{c.filename}</span>
            </button>
          ))}
        </div>
      )}
    </div>
  );
}

export default function ChatPage() {
  const [input, setInput] = useState("");
  const [chats, setChats] = useState<DbChat[]>([]);
  const [llm, setLlm] = useState<LlmSettings | null>(null);
  const inputRef = useRef<HTMLTextAreaElement>(null);
  const scrollRef = useRef<HTMLDivElement>(null);

  const { chatId, messages, streaming, tools, sending, error, model } = useChatStore();
  const store = useChatStore;

  useEffect(() => {
    inputRef.current?.focus();
    getChats().then(setChats).catch(() => {});
    // The switcher starts on the configured chat model and stays wherever the
    // user left it for the rest of the session.
    getLlmSettings().then((s) => {
      setLlm(s);
      if (!store.getState().model && s.chatModel) store.getState().setModel(s.chatModel);
    });
  }, [store]);

  // Live turn events. Scoped to this page: a stream only matters while it is
  // on screen, and every message is persisted by Rust regardless.
  useEffect(() => {
    const unsubs = [
      listen<{ chatId: number; delta: string }>("chat-delta", (e) => {
        if (e.payload.chatId === store.getState().chatId) {
          store.getState().appendDelta(e.payload.delta);
        }
      }),
      listen<{ chatId: number; name: string; args: any; status: string }>("chat-tool", (e) => {
        if (e.payload.chatId !== store.getState().chatId) return;
        const a = e.payload.args ?? {};
        const detail = a.query ?? a.relative_path ?? "";
        store.getState().toolEvent(e.payload.name, detail, e.payload.status);
      }),
      listen<{ chatId: number; message: any }>("chat-message", (e) => {
        if (e.payload.chatId !== store.getState().chatId) return;
        const m = e.payload.message;
        store.getState().commit({
          id: m.id,
          chat_id: e.payload.chatId,
          role: "assistant",
          content: m.content,
          tool_calls: null,
          tool_call_id: null,
          citations: m.citations ?? null,
          model: m.model ?? null,
          created_at: new Date().toISOString(),
        });
      }),
      listen<{ chatId: number }>("chat-done", (e) => {
        if (e.payload.chatId === store.getState().chatId) {
          store.getState().finish();
          getChats().then(setChats).catch(() => {});
        }
      }),
      listen<{ chatId: number; error: string }>("chat-error", (e) => {
        if (e.payload.chatId === store.getState().chatId) store.getState().fail(e.payload.error);
      }),
    ];
    return () => {
      unsubs.forEach((u) => u.then((f) => f()));
    };
  }, [store]);

  useEffect(() => {
    scrollRef.current?.scrollTo({ top: scrollRef.current.scrollHeight });
  }, [messages.length, streaming, tools.length]);

  async function send(text: string) {
    const content = text.trim();
    if (!content || sending) return;
    setInput("");
    const s = store.getState();
    s.commit(localUserMessage(s.chatId, content));
    s.begin(s.chatId ?? -1);
    try {
      const id = await invoke<number>("chat_send", {
        chatId: s.chatId,
        content,
        model: s.model,
      });
      store.setState({ chatId: id });
      getChats().then(setChats).catch(() => {});
    } catch (e) {
      store.getState().fail(String(e));
    }
  }

  const empty = messages.length === 0 && !streaming && !sending;

  /* One composer, rendered in one of two places: centred under the hero on an
     empty chat, or docked at the bottom once there is a transcript to sit
     under. Keeping it a single element means the textarea keeps its ref,
     focus and draft text across that move. */
  const composer = (
    <div className="flex flex-col gap-2 rounded-2xl border border-border bg-card px-4 py-3.5 shadow-sm transition-[border-color,box-shadow] focus-within:border-ring focus-within:ring-[3px] focus-within:ring-ring/25">
      <Textarea
        ref={inputRef}
        value={input}
        onChange={(e) => setInput(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === "Enter" && !e.shiftKey) {
            e.preventDefault();
            send(input);
          }
        }}
        rows={1}
        placeholder="Ask about your courses, deadlines, lectures…"
        className="min-h-[20px] max-h-[160px] w-full resize-none rounded-none border-0 bg-transparent p-0 text-sm leading-5 shadow-none focus-visible:border-0 focus-visible:ring-0 dark:bg-transparent"
        style={{ height: "20px" }}
        onInput={(e) => {
          const el = e.currentTarget;
          el.style.height = "20px";
          el.style.height = `${Math.min(el.scrollHeight, 160)}px`;
        }}
      />
      <div className="flex items-center gap-2">
        <ModelSelect
          library={llm?.library ?? []}
          providers={llm?.providers ?? []}
          value={model}
          onChange={(m) => store.getState().setModel(m)}
          placeholder="Model"
          className="h-6 max-w-[280px] border-0 bg-transparent px-1 text-[11px] text-muted-foreground shadow-none dark:bg-transparent"
        />
        <div className="flex-1" />
        {sending ? (
          <Button
            size="icon-sm"
            variant="ghost"
            className="shrink-0"
            aria-label="Stop"
            onClick={() =>
              chatId != null && invoke("chat_cancel", { chatId }).catch(() => {})
            }
          >
            <Stop size={14} />
          </Button>
        ) : (
          <Button
            size="icon-sm"
            disabled={!input.trim()}
            onClick={() => send(input)}
            className="shrink-0"
            aria-label="Send"
          >
            <PaperPlaneTilt size={14} />
          </Button>
        )}
      </div>
    </div>
  );

  return (
    <div className="flex h-full">
      {/* Conversations */}
      <aside className="w-52 shrink-0 border-r border-border-subtle flex flex-col">
        <div className="p-2">
          <Button
            variant="ghost"
            size="xs"
            className="w-full justify-start"
            onClick={() => store.getState().open(null)}
          >
            <Plus size={13} /> New chat
          </Button>
        </div>
        <div className="flex-1 overflow-y-auto px-2 pb-2 flex flex-col gap-0.5">
          {chats.map((c) => (
            <button
              key={c.id}
              type="button"
              onClick={() => store.getState().open(c.id)}
              className={cn(
                "rounded-lg px-2.5 py-1.5 text-left text-xs truncate transition-colors",
                c.id === chatId
                  ? "bg-accent text-foreground"
                  : "text-muted-foreground hover:bg-accent hover:text-foreground",
              )}
            >
              {c.title || "Untitled"}
            </button>
          ))}
        </div>
      </aside>

      <div className="flex flex-1 flex-col min-w-0">
        <div className="flex items-center justify-between px-6 h-12 border-b border-border-subtle shrink-0">
          <span className="font-display font-semibold text-[13px] text-foreground">Chat</span>
        </div>

        {empty && !error ? (
          /* Nothing said yet: the page is a title and a place to type, the
             way a blank document is. The transcript layout only appears once
             there is a transcript. */
          <div className="flex-1 overflow-y-auto px-6">
            <div className="mx-auto flex min-h-full w-full max-w-2xl flex-col items-center justify-center gap-7 pb-16">
              <div className="flex flex-col items-center gap-4">
                <h1 className="text-display text-foreground">Ask Oculus anything</h1>
                <p className="max-w-md text-center text-[13px] leading-relaxed text-muted-foreground">
                  It searches your subjects — pages, slides, lecture transcripts,
                  Ed threads — and answers with the file it read.
                </p>
              </div>

              <div className="w-full">{composer}</div>

              <div className="flex flex-wrap justify-center gap-2">
                {SUGGESTIONS.map((sugg) => (
                  <button
                    key={sugg}
                    type="button"
                    onClick={() => send(sugg)}
                    className="rounded-full border border-border bg-card px-3.5 py-1.5 text-[12px] text-muted-foreground transition-colors hover:border-surface-overlay hover:bg-accent hover:text-foreground"
                  >
                    {sugg}
                  </button>
                ))}
              </div>
            </div>
          </div>
        ) : (
          <>
            <div ref={scrollRef} className="flex-1 overflow-y-auto px-6 py-6">
              <div className="max-w-2xl mx-auto flex flex-col gap-5">
                {messages.map((m) => (
                  <Message key={m.id} m={m} />
                ))}

                {tools.map((t, i) => (
                  <div
                    key={`${t.name}-${i}`}
                    className="flex items-center gap-2 text-xs text-muted-foreground"
                  >
                    {t.done ? (
                      <MagnifyingGlass size={12} className="shrink-0" />
                    ) : (
                      <CircleNotch size={12} className="shrink-0 animate-spin" />
                    )}
                    <span className="truncate">
                      {TOOL_LABEL[t.name] ?? t.name}
                      {t.detail && <span className="text-muted-foreground/70"> — {t.detail}</span>}
                    </span>
                  </div>
                ))}

                {streaming && (
                  <ReactMarkdown
                    remarkPlugins={[remarkGfm, remarkMath]}
                    rehypePlugins={[rehypeKatex]}
                    components={CHAT_MD}
                  >
                    {streaming}
                  </ReactMarkdown>
                )}

                {sending && !streaming && tools.length === 0 && (
                  <CircleNotch size={14} className="animate-spin text-muted-foreground" />
                )}

                {error && (
                  <Alert variant="destructive">
                    <AlertDescription>{error}</AlertDescription>
                  </Alert>
                )}
              </div>
            </div>

            <div className="px-6 pb-5 pt-2 shrink-0">
              <div className="mx-auto max-w-2xl">{composer}</div>
            </div>
          </>
        )}
      </div>
    </div>
  );
}
