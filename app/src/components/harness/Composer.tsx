import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { FileText, PaperPlaneTilt, Stop } from "@phosphor-icons/react";
import { Button } from "@/components/ui/button";
import { Textarea } from "@/components/ui/textarea";
import { ModelPicker, type PickerProvider } from "@/components/harness/ModelPicker";
import { SubjectSelect } from "@/components/harness/SubjectSelect";
import { fileTitle } from "@/lib/openFile";
import { displayCode } from "@/lib/format";
import { searchMentionFiles, type MentionFile, type Subject } from "@/lib/db";
import {
  CLAUDE_MODELS,
  PROVIDERS,
  codexAsModels,
  defaultSelection,
  harnessCodexModels,
  type CodexModel,
  type Provider,
  type RateWindow,
  type ThreadUsage,
} from "@/lib/harness";
import { cn } from "@/lib/utils";

/** How much of an `@` token to look at. Long enough for a real filename,
 *  short enough that a stray `@` in prose stops matching once the sentence
 *  runs on. */
const MAX_MENTION = 60;

/**
 * The `@…` token the caret is sitting in, or null.
 *
 * Anchored to the start of a word so an email address or a handle typed
 * mid-word never opens the menu, and ended by whitespace so the token is
 * whatever was typed after the `@`.
 */
export function mentionQuery(
  text: string,
  caret: number,
): { query: string; start: number } | null {
  const m = new RegExp(`(?:^|\\s)@([^\\s@]{0,${MAX_MENTION}})$`).exec(text.slice(0, caret));
  return m ? { query: m[1], start: caret - m[1].length - 1 } : null;
}

/**
 * One box: the text, then a row of pickers and the send/stop button, then a
 * footer line with what the thread has cost so far. Enter sends, Shift+Enter
 * breaks a line — bb's keys. While a turn runs the send button becomes stop
 * and the text stays put; steering mid-turn is not wired yet.
 *
 * `@` opens a file menu, narrowed to the thread's subject. Picking a file
 * writes its **library path** into the message — nothing is read here and no
 * content is attached. The agent has the library in front of it and its own
 * tools for opening a file; a path is all it was ever missing, and one it can
 * hand straight to `oculus read`.
 */
export function Composer({
  provider,
  model,
  reasoning,
  providerLocked,
  subjects,
  subjectId,
  onSubject,
  subjectLocked,
  running,
  usage,
  rateLimits,
  onProvider,
  onModel,
  onReasoning,
  onSend,
  onStop,
  autoFocus,
}: {
  provider: Provider;
  model: string | null;
  /** Reasoning effort for the next turn; null leaves the flag off. */
  reasoning: string | null;
  /** An open thread keeps its provider; only a new one can pick. */
  providerLocked: boolean;
  subjects: Subject[];
  /** null is the general thread — the whole library. */
  subjectId: number | null;
  onSubject: (id: number | null) => void;
  /** An open thread keeps its scope too, for the same reason. */
  subjectLocked: boolean;
  running: boolean;
  usage: ThreadUsage | null;
  rateLimits: RateWindow[];
  onProvider: (p: Provider) => void;
  onModel: (m: string | null) => void;
  onReasoning: (level: string | null) => void;
  onSend: (text: string) => void;
  onStop: () => void;
  autoFocus?: boolean;
}) {
  const [text, setText] = useState("");
  const ref = useRef<HTMLTextAreaElement>(null);
  const [codexModels, setCodexModels] = useState<CodexModel[] | null>(null);
  const [mention, setMention] = useState<{ query: string; start: number } | null>(null);
  const [files, setFiles] = useState<MentionFile[]>([]);
  const [index, setIndex] = useState(0);
  // The query the menu is currently showing, so a slow lookup that lands
  // after the token changed cannot overwrite a newer list.
  const latest = useRef("");
  /** Where the caret goes once the picked path is in the DOM. */
  const caretAfterPick = useRef<number | null>(null);

  useEffect(() => {
    if (autoFocus) ref.current?.focus();
  }, [autoFocus]);

  // Codex lists its own models (`model/list`), fetched once the picker is
  // for Codex; Claude's are the CLI's aliases.
  useEffect(() => {
    if (provider !== "codex" || codexModels) return;
    harnessCodexModels().then(setCodexModels).catch(() => setCodexModels([]));
  }, [provider, codexModels]);

  useEffect(() => {
    if (!mention) {
      setFiles([]);
      return;
    }
    const token = `${subjectId ?? ""} ${mention.query}`;
    latest.current = token;
    searchMentionFiles(subjectId, mention.query)
      .then((found) => {
        if (latest.current !== token) return;
        setFiles(found);
        setIndex(0);
      })
      .catch(() => {});
  }, [mention, subjectId]);

  // A subject change re-scopes what `@` may reach, so the open list is stale.
  useEffect(() => setMention(null), [subjectId]);

  const pickerProviders: PickerProvider[] = PROVIDERS.map((p) =>
    p.id === "claude"
      ? { ...p, models: CLAUDE_MODELS }
      : { ...p, models: codexAsModels(codexModels ?? []), loading: codexModels === null },
  );

  // No turn goes out without a model and a level, so an empty selection —
  // Codex before its CLI has answered — is filled the moment a list exists.
  const active = pickerProviders.find((p) => p.id === provider);
  useEffect(() => {
    if (model || !active || active.loading || active.models.length === 0) return;
    const pick = defaultSelection(active.models);
    if (!pick.model) return;
    onModel(pick.model);
    onReasoning(pick.reasoning);
  }, [model, active, onModel, onReasoning]);

  /** Recompute the token from wherever the caret actually is: typing, but
   *  also an arrow key or a click that lands beside an existing `@`. */
  function syncMention(el: HTMLTextAreaElement) {
    setMention(mentionQuery(el.value, el.selectionStart ?? el.value.length));
  }

  /**
   * The box grows to its content after every change, and a pick puts the
   * caret back after the path it inserted.
   *
   * Both belong here rather than in the handlers that cause them: a pick
   * writes through React, so at the moment it runs — and in a `requestAnimationFrame`
   * after it — the textarea still holds the old text, and measuring or
   * addressing it then sizes the box to the wrong string. A layout effect is
   * the first point at which the DOM says what the state does.
   */
  useLayoutEffect(() => {
    const el = ref.current;
    if (!el) return;
    el.style.height = "20px";
    el.style.height = `${Math.min(el.scrollHeight, 200)}px`;
    if (caretAfterPick.current != null) {
      el.focus();
      el.setSelectionRange(caretAfterPick.current, caretAfterPick.current);
      caretAfterPick.current = null;
    }
  }, [text]);

  /** Swap the `@token` for the file's library path, fenced so it reads as a
   *  path rather than as part of the sentence. */
  function pick(file: MentionFile) {
    const el = ref.current;
    if (!el || !mention) return;
    const caret = el.selectionStart ?? text.length;
    const token = `\`${file.relative_path}\` `;
    setText(text.slice(0, mention.start) + token + text.slice(caret));
    caretAfterPick.current = mention.start + token.length;
    setMention(null);
  }

  const menuOpen = mention !== null && files.length > 0;

  const send = () => {
    const t = text.trim();
    if (!t || running) return;
    setText("");
    setMention(null);
    onSend(t);
  };

  return (
    <div className="relative flex flex-col gap-1.5">
      {menuOpen && (
        <div className="absolute bottom-full left-0 right-0 z-20 mb-2 overflow-hidden rounded-xl border border-border bg-popover py-1 shadow-md">
          {files.map((f, i) => (
            <button
              key={f.id}
              type="button"
              onMouseDown={(e) => {
                e.preventDefault();
                pick(f);
              }}
              onMouseEnter={() => setIndex(i)}
              className={cn(
                "flex w-full items-center gap-2 px-3 py-1.5 text-left text-xs",
                i === index ? "bg-accent text-foreground" : "text-muted-foreground",
              )}
            >
              <FileText size={12} className="shrink-0" />
              <span className="truncate">{fileTitle(f)}</span>
              {subjectId == null && (
                <span className="ml-auto shrink-0 text-[10px] text-muted-foreground/70">
                  {displayCode(f.subject_code)}
                </span>
              )}
            </button>
          ))}
        </div>
      )}

      <div className="flex flex-col gap-2 rounded-2xl border border-border bg-card px-4 py-3.5 shadow-sm transition-[border-color,box-shadow] focus-within:border-ring focus-within:ring-[3px] focus-within:ring-ring/25">
        <Textarea
          ref={ref}
          value={text}
          onChange={(e) => {
            setText(e.target.value);
            syncMention(e.currentTarget);
          }}
          onKeyUp={(e) => syncMention(e.currentTarget)}
          onClick={(e) => syncMention(e.currentTarget)}
          onBlur={() => setMention(null)}
          onKeyDown={(e) => {
            if (menuOpen) {
              if (e.key === "ArrowDown") {
                e.preventDefault();
                setIndex((i) => (i + 1) % files.length);
                return;
              }
              if (e.key === "ArrowUp") {
                e.preventDefault();
                setIndex((i) => (i - 1 + files.length) % files.length);
                return;
              }
              if (e.key === "Enter" || e.key === "Tab") {
                e.preventDefault();
                pick(files[index]);
                return;
              }
              if (e.key === "Escape") {
                e.preventDefault();
                setMention(null);
                return;
              }
            }
            if (e.key === "Enter" && !e.shiftKey) {
              e.preventDefault();
              send();
            }
          }}
          rows={1}
          placeholder={
            running
              ? "Working… stop to send another message"
              : "Ask about your subjects, or give the agent a task… @ for a file"
          }
          className="min-h-[20px] max-h-[200px] w-full resize-none rounded-none border-0 bg-transparent p-0 text-sm leading-5 shadow-none focus-visible:border-0 focus-visible:ring-0 dark:bg-transparent"
          style={{ height: "20px" }}
        />
        <div className="flex items-center gap-1">
          <SubjectSelect
            subjects={subjects}
            value={subjectId}
            onChange={onSubject}
            disabled={subjectLocked}
            className="h-6 max-w-[200px] border-0 bg-transparent px-1.5 text-[11px] text-muted-foreground shadow-none hover:bg-accent disabled:opacity-100 dark:bg-transparent"
          />
          <ModelPicker
            providers={pickerProviders}
            provider={provider}
            providerLocked={providerLocked}
            model={model}
            reasoning={reasoning}
            onProvider={onProvider}
            onModel={onModel}
            onReasoning={onReasoning}
          />
          <div className="flex-1" />
          {running ? (
            <Button size="icon-sm" variant="ghost" className="shrink-0" aria-label="Stop" onClick={onStop}>
              <Stop size={14} weight="fill" />
            </Button>
          ) : (
            <Button size="icon-sm" disabled={!text.trim()} onClick={send} className="shrink-0" aria-label="Send">
              <PaperPlaneTilt size={14} />
            </Button>
          )}
        </div>
      </div>
      <Footer usage={usage} rateLimits={rateLimits} />
    </div>
  );
}

function fmtTokens(n: number) {
  return n >= 1000 ? `${Math.round(n / 1000)}k` : String(n);
}

/** Context used, spend, and the account's windows — one quiet line. */
function Footer({ usage, rateLimits }: { usage: ThreadUsage | null; rateLimits: RateWindow[] }) {
  const parts: string[] = [];
  if (usage?.contextTokens) {
    parts.push(
      usage.contextWindow
        ? `${fmtTokens(usage.contextTokens)} / ${fmtTokens(usage.contextWindow)} context`
        : `${fmtTokens(usage.contextTokens)} context tokens`,
    );
  }
  if (usage?.costUsd) parts.push(`$${usage.costUsd.toFixed(2)}`);
  for (const w of rateLimits) parts.push(`${w.label} ${Math.round(w.used_percent)}%`);
  if (parts.length === 0) return null;
  return (
    <div className={cn("flex min-h-4 items-center gap-3 px-4 text-[11px] tabular-nums text-muted-foreground")}>
      {parts.map((p, i) => (
        <span key={i}>{p}</span>
      ))}
    </div>
  );
}
