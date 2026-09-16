import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { FileText, PaperPlaneTilt, Stop } from "@phosphor-icons/react";
import { Button } from "@/components/ui/button";
import { Textarea } from "@/components/ui/textarea";
import { ModelPicker } from "@/components/harness/ModelPicker";
import { UsageMeter } from "@/components/harness/UsageMeter";
import { SubjectSelect } from "@/components/harness/SubjectSelect";
import { fileTitle } from "@/lib/openFile";
import { displayCode } from "@/lib/format";
import {
  countUnparsedMentionMatches,
  searchMentionFiles,
  type MentionFile,
  type Subject,
} from "@/lib/db";
import { useParseStore } from "@/stores/parseStore";
import { PARSE_SWEEP_NOTE } from "@/lib/parseState";
import {
  defaultSelection,
  type Provider,
  type RateWindow,
  type ThreadUsage,
} from "@/lib/harness";
import { useProviderModels } from "@/hooks/useProviderModels";
import { cn } from "@/lib/utils";

/** How much of an `@` token to look at. Long enough for a real filename,
 *  short enough that a stray `@` in prose stops matching once the sentence
 *  runs on. */
const MAX_MENTION = 60;

/** The empty box is exactly one line tall. Both halves of the autosize below
 *  measure against this, so it has to stay the textarea's own line-height. */
const LINE_H = 16;

/** Roughly ten lines. Past that the box stops growing and scrolls. */
const MAX_H = 160;

/** The gap the `@` menu keeps from the composer and from the viewport edge —
 *  `mb-2`/`mt-2`, in pixels, because the flip below has to do arithmetic with
 *  it. */
const MENU_GAP = 8;

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
 * One box: the text, then a row carrying the model picker on the left and the
 * usage wheel and send/stop button on the right. Enter sends, Shift+Enter
 * breaks a line — bb's keys.
 *
 * While a turn runs the box still takes a message: it is *queued* rather than
 * sent (Rust holds it — `Queue` in `app/src-tauri/src/harness/mod.rs`) and it
 * appears as a pending bubble at the end of the thread. Stop then keeps its
 * word — nothing more goes out — and hands back what was waiting, which
 * arrives here as `restore` and lands in the box rather than being lost.
 * So while a turn runs there are two buttons and not one: stop is always
 * reachable, and send appears beside it as soon as there is something to
 * queue.
 *
 * The row's controls are all one height (24px). A send button larger than the
 * picker beside it reads as the box's subject rather than its verb.
 *
 * Scope sits *above* the box rather than in the control row, and only while
 * the thread is new: it is a choice made once, before the first message, and
 * an open thread cannot change it. Showing a dead control under every message
 * for the rest of the thread spends the row on nothing.
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
  restore,
  onRestored,
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
  /** Messages stop dropped out of the queue, handed back to be typed over.
   *  The counter is what is watched: the same text twice is two restores. */
  restore?: { text: string; n: number } | null;
  onRestored?: () => void;
  onProvider: (p: Provider) => void;
  onModel: (m: string | null) => void;
  onReasoning: (level: string | null) => void;
  onSend: (text: string) => void;
  onStop: () => void;
  autoFocus?: boolean;
}) {
  const [text, setText] = useState("");
  const ref = useRef<HTMLTextAreaElement>(null);
  const wrapRef = useRef<HTMLDivElement>(null);
  const menuRef = useRef<HTMLDivElement>(null);
  const [mention, setMention] = useState<{ query: string; start: number } | null>(null);
  const [files, setFiles] = useState<MentionFile[]>([]);
  /** Files this query matched that have no markdown, so could not be offered. */
  const [unparsed, setUnparsed] = useState(0);
  const [index, setIndex] = useState(0);
  /** Which way the `@` menu opens. See the layout effect that sets it. */
  const [drop, setDrop] = useState<"up" | "down">("down");
  // The query the menu is currently showing, so a slow lookup that lands
  // after the token changed cannot overwrite a newer list.
  const latest = useRef("");
  /** Where the caret goes once the picked path is in the DOM. */
  const caretAfterPick = useRef<number | null>(null);
  /** An app-wide parse failure, so the line below can name the real cause
   *  rather than blaming the files. */
  const latch = useParseStore((s) => s.latch);

  useEffect(() => {
    if (autoFocus) ref.current?.focus();
  }, [autoFocus]);

  // What stop gave back. It is put *before* anything already typed, because
  // it was typed first, and the box is focused so the next key carries on
  // where the student left off.
  useEffect(() => {
    if (!restore) return;
    setText((t) => [restore.text, t].filter(Boolean).join("\n\n"));
    ref.current?.focus();
    onRestored?.();
  }, [restore, onRestored]);

  useEffect(() => {
    if (!mention) {
      setFiles([]);
      setUnparsed(0);
      return;
    }
    const token = `${subjectId ?? ""} ${mention.query}`;
    latest.current = token;
    searchMentionFiles(subjectId, mention.query)
      .then(async (found) => {
        if (latest.current !== token) return;
        setFiles(found);
        setIndex(0);
        // Only when the menu has nothing to show: an empty type-ahead reads
        // as a typo, and for a PDF that is simply unparsed it is not one.
        // Counting on every keystroke would be a second query per key for a
        // line almost nobody ever sees.
        const missing = found.length === 0
          ? await countUnparsedMentionMatches(subjectId, mention.query).catch(() => 0)
          : 0;
        if (latest.current === token) setUnparsed(missing);
      })
      .catch(() => {});
  }, [mention, subjectId]);

  // A subject change re-scopes what `@` may reach, so the open list is stale.
  useEffect(() => setMention(null), [subjectId]);

  // Only the provider the picker is on is worth asking a CLI about: a
  // composer that is on Claude should not spawn the others for lists nobody
  // has opened the menu to see.
  const { providers: pickerProviders } = useProviderModels(provider);

  // No turn goes out without a model and a level, so an empty selection — a
  // fetched catalogue before its CLI has answered — is filled the moment a
  // list exists.
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
    el.style.height = `${LINE_H}px`;
    el.style.height = `${Math.min(el.scrollHeight, MAX_H)}px`;
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
  /**
   * Why the menu is empty, when the answer is "those files have no markdown".
   *
   * Deliberately *not* rows in the menu: a row that cannot be picked is a
   * control that does nothing, and the agent genuinely cannot read a file that
   * was never parsed. It is a line instead — and it is outside `menuOpen`, so
   * the list's keyboard handling, its flip measurement and Enter-sends all
   * behave exactly as they do with no `@` open at all.
   */
  const emptyReason = mention !== null && files.length === 0 && unparsed > 0;

  /**
   * The `@` menu opens downwards, and only flips up when it would not fit.
   *
   * The composer is the same component in three very different places — the
   * middle of the home page, the middle of the chat hero, and pinned to the
   * bottom of an open thread — so which way is "out of the way" is a fact
   * about the viewport, not about the call site. Opening up unconditionally
   * was right for the thread and wrong everywhere else: on the home page the
   * list covered the subject pill and the cards above it while the whole
   * lower half of the page sat empty.
   *
   * Measured after the menu is in the DOM rather than against `max-h-64`, so
   * a three-file list is judged on the ~90px it actually occupies instead of
   * the 256px it is allowed. `useLayoutEffect`, so the flip lands before
   * paint and the menu is never seen in the wrong place.
   */
  useLayoutEffect(() => {
    const wrap = wrapRef.current;
    const menu = menuRef.current;
    if (!menuOpen || !wrap || !menu) return;
    const box = wrap.getBoundingClientRect();
    const needed = menu.offsetHeight + MENU_GAP;
    setDrop(window.innerHeight - box.bottom >= needed || box.top < needed ? "down" : "up");
  }, [menuOpen, files.length]);

  const send = () => {
    const t = text.trim();
    if (!t) return;
    setText("");
    setMention(null);
    // Whether this goes out now or waits behind the running turn is Rust's
    // call, not this box's: it owns the queue and the order.
    onSend(t);
  };

  return (
    <div ref={wrapRef} className="relative flex flex-col gap-1.5">
      {!subjectLocked && (
        <div className="flex items-center px-0.5">
          <SubjectSelect
            subjects={subjects}
            value={subjectId}
            onChange={onSubject}
            className="h-6 max-w-[200px] rounded-full border-border/70 bg-card px-2 text-[11px] text-muted-foreground shadow-none hover:bg-accent hover:text-foreground"
          />
        </div>
      )}

      {emptyReason && (
        <div
          className={cn(
            "absolute left-0 right-0 z-20 rounded-xl border border-border bg-popover px-3 py-2 text-[11px] leading-snug text-muted-foreground shadow-md",
            drop === "down" ? "top-full mt-2" : "bottom-full mb-2",
          )}
        >
          {unparsed === 1
            ? "1 matching file has no markdown, so it cannot be mentioned."
            : `${unparsed} matching files have no markdown, so they cannot be mentioned.`}
          {` ${latch ? latch.message : PARSE_SWEEP_NOTE}`}
        </div>
      )}

      {menuOpen && (
        <div
          ref={menuRef}
          className={cn(
            "absolute left-0 right-0 z-20 max-h-64 overflow-y-auto overflow-x-hidden rounded-xl border border-border bg-popover py-1 shadow-md",
            drop === "down" ? "top-full mt-2" : "bottom-full mb-2",
          )}
        >
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

      <div className="flex flex-col gap-4 rounded-xl border border-border bg-card px-3 py-3 shadow-sm transition-[border-color,box-shadow] focus-within:border-ring focus-within:ring-[3px] focus-within:ring-ring/25">
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
            running ? "Working… your next message waits its turn" : "What would you like to work on?"
          }
          className="min-h-[16px] max-h-[160px] w-full resize-none rounded-none border-0 bg-transparent p-0 text-[13px]! leading-[16px] shadow-none focus-visible:border-0 focus-visible:ring-0 dark:bg-transparent"
          style={{ height: `${LINE_H}px` }}
        />
        <div className="flex items-center gap-1">
          <ModelPicker
            className="-ml-1.5"
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
          <UsageMeter usage={usage} rateLimits={rateLimits} />
          {running && (
            <Button size="icon-xs" variant="ghost" className="shrink-0" aria-label="Stop" onClick={onStop}>
              <Stop weight="fill" />
            </Button>
          )}
          {(!running || text.trim()) && (
            <Button
              size="icon-xs"
              disabled={!text.trim()}
              onClick={send}
              className="shrink-0"
              aria-label={running ? "Queue" : "Send"}
            >
              <PaperPlaneTilt />
            </Button>
          )}
        </div>
      </div>
    </div>
  );
}
