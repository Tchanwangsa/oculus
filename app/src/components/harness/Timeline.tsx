import { memo, useLayoutEffect, useMemo, useRef, useState } from "react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import remarkMath from "remark-math";
import rehypeKatex from "rehype-katex";
import {
  ArrowClockwise,
  ArrowCounterClockwise,
  CaretDown,
  Check,
  CircleNotch,
  Copy,
  PencilSimple,
  X,
} from "@phosphor-icons/react";
import type { Icon } from "@phosphor-icons/react";
import { MATH, MD_COMPONENTS, normalizeMath } from "@/components/markdown/MdComponents";
import { FileChip } from "@/components/markdown/FileChip";
import { openLibraryPath, splitLibraryPaths, type TextPart } from "@/lib/openFile";
import { attachmentSrc } from "@/lib/attachments";
import { ImageLightbox } from "@/components/ui/Lightbox";
import { selectionMarkdown } from "@/lib/selectionMarkdown";
import { useDataDir } from "@/hooks/useDataDir";
import { Button } from "@/components/ui/button";
import { Textarea } from "@/components/ui/textarea";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { fmtClock, sqliteUtcToMs } from "@/lib/format";
import { fmtTime } from "@/lib/lectures";
import {
  messageAt,
  parseErrorMeta,
  parseToolMeta,
  type HarnessItem,
  type Provider,
  type ToolKind,
} from "@/lib/harness";
import { useSignInStatus } from "@/hooks/useSignInStatus";
import { useHarnessStore } from "@/stores/harnessStore";
import { cn, copyText } from "@/lib/utils";
import { ErrorRow, RowShell, ThinkingRow, ToolRow, TOOL_ICON } from "./WorkRow";
import { PermissionCard } from "./PermissionCard";
import { SignInDialog, useSignIn } from "./SignInDialog";

/** Editing a question, or one still waiting to be asked, both happen in the
 *  bubble itself rather than back in the composer: the thread is where the
 *  question is, and a box that opened somewhere else would lose its place in
 *  the conversation it is being asked about.
 *
 *  The three that truncate the thread — edit, rewind, retry — are absent for
 *  a provider that cannot take a question back out of its own context
 *  (`ProviderInfo.rewind`): offering them there would shorten the screen and
 *  leave the agent remembering what it no longer shows. */
export interface QuestionActions {
  /** Ask it again, differently. The thread rewinds to this row. */
  edit?: (itemId: number, text: string) => void;
  /** Take the thread back to just before this question and hand its words to
   *  the composer. Claude Code's rewind, without the branching. */
  rewind?: (itemId: number) => void;
  /** Ask the same question again, unchanged — what Retry under an answer is. */
  retry?: (itemId: number, text: string) => void;
  /** Send a new message on the open thread, on the thread's own model and
   *  the composer's level — what an approved permission carries on with. */
  followUp: (text: string) => Promise<void>;
}

/**
 * The thread as a list. Messages are the spine; the work between them —
 * tool calls, reasoning — is a *step*, and a finished step with more than
 * one row folds into a single summary row ("Explored 3 files, ran 2
 * commands") that opens to the rows. The step still in progress stays
 * unfolded, with its rows at full strength; finished rows are dimmed. This is
 * bb's `buildTimelineViewRows`, with one level of grouping instead of two.
 *
 * **Nothing here re-renders for the turn in flight.** A committed row is a row
 * Rust wrote and will not change again, so every one of them is memoised, and
 * the two things that do change while a turn runs subscribe to the store
 * themselves: the tail below, and the one tool row whose output is still
 * arriving. Re-parsing a thread's markdown on every streamed token was the
 * whole of the jitter — a thread's worth costs ~15ms, which is a dropped
 * frame per token when the tree above the stream is rebuilt to show it.
 */

type ViewRow =
  | { kind: "item"; item: HarnessItem; dim: boolean }
  | { kind: "bundle"; id: string; items: HarnessItem[] };

const isWork = (i: HarnessItem) => i.kind === "tool" || i.kind === "thinking";

function buildRows(items: HarnessItem[], running: boolean): ViewRow[] {
  const out: ViewRow[] = [];
  let step: HarnessItem[] = [];
  const close = (dim: boolean) => {
    if (step.length >= 2 && dim) out.push({ kind: "bundle", id: `b-${step[0].id}`, items: step });
    else for (const item of step) out.push({ kind: "item", item, dim });
    step = [];
  };
  for (const item of items) {
    if (isWork(item)) step.push(item);
    else {
      close(true);
      out.push({ kind: "item", item, dim: false });
    }
  }
  // The trailing step is live while the turn runs; otherwise it is as
  // finished as the rest.
  close(!running);
  return out;
}

function plural(n: number, one: string, many = `${one}s`) {
  return `${n} ${n === 1 ? one : many}`;
}

/** "Explored 3 files, 2 searches, ran 1 command" */
function bundleLabel(items: HarnessItem[]): { label: string; icon: ToolKind } {
  const counts: Partial<Record<ToolKind | "thinking", number>> = {};
  for (const i of items) {
    const k = i.kind === "thinking" ? "thinking" : (parseToolMeta(i).kind ?? "other");
    counts[k] = (counts[k] ?? 0) + 1;
  }
  const parts: string[] = [];
  const reads = counts.read ?? 0;
  if (reads) parts.push(`explored ${plural(reads, "file")}`);
  if (counts.search) parts.push(plural(counts.search, "search", "searches"));
  if (counts.oculus_cli) parts.push(`looked up the library ${counts.oculus_cli === 1 ? "once" : `${counts.oculus_cli} times`}`);
  if (counts.bash) parts.push(`ran ${plural(counts.bash, "command")}`);
  const edits = (counts.edit ?? 0) + (counts.write ?? 0);
  if (edits) parts.push(`edited ${plural(edits, "file")}`);
  if (counts.web) parts.push(plural(counts.web, "web lookup"));
  if (counts.task) parts.push(`ran ${plural(counts.task, "subagent")}`);
  if (counts.plan) parts.push("updated the plan");
  if (counts.other) parts.push(plural(counts.other, "tool call"));
  if (counts.thinking && parts.length === 0) parts.push("thought");
  const label = parts.join(", ");
  const dominant = (Object.entries(counts) as [ToolKind | "thinking", number][])
    .filter(([k]) => k !== "thinking")
    .sort((a, b) => b[1] - a[1])[0]?.[0] as ToolKind | undefined;
  return { label: label.charAt(0).toUpperCase() + label.slice(1), icon: dominant ?? "other" };
}

/** The two plugin lists, frozen: a reply with no maths in it keeps prop
 *  identity across renders, and `MATH` (`MdComponents`) is what decides which
 *  it gets — KaTeX is the most expensive thing in the pipeline and most
 *  replies carry no delimiter at all. */
const PLAIN = [remarkGfm];
const WITH_MATH = [remarkGfm, remarkMath];
const KATEX = [rehypeKatex];
const NO_PLUGINS: never[] = [];

/** `md-compact` scales the shared markdown components down to a panel's size —
 *  see the rule in `app/src/index.css`. The components themselves are sized
 *  for a document (the file viewer), which is a size too large for a reply. */
const Assistant = memo(function Assistant({ text }: { text: string }) {
  const math = MATH.test(text);
  const body = math ? normalizeMath(text) : text;
  return (
    <div className="md-compact min-w-0 px-2 text-[13px] leading-relaxed">
      <ReactMarkdown
        remarkPlugins={math ? WITH_MATH : PLAIN}
        rehypePlugins={math ? KATEX : NO_PLUGINS}
        components={MD_COMPONENTS}
      >
        {body}
      </ReactMarkdown>
    </div>
  );
});

/** One action under a message: an icon and the word for it, nothing else.
 *  The row they sit in is revealed by hovering the message, so at rest a
 *  thread is still only what was said. */
function Action({
  label,
  icon: Icon,
  onClick,
}: {
  label: string;
  icon: Icon;
  onClick: () => void;
}) {
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <button
          type="button"
          aria-label={label}
          onClick={onClick}
          className="cursor-pointer rounded-full p-1.5 text-muted-foreground transition-colors hover:bg-accent hover:text-foreground"
        >
          <Icon size={14} />
        </button>
      </TooltipTrigger>
      <TooltipContent>{label}</TooltipContent>
    </Tooltip>
  );
}

/** Copy is the one action that answers for itself: the icon becomes a tick
 *  rather than a toast, which this app does not have. */
function CopyAction({ text }: { text: string }) {
  const [done, setDone] = useState(false);
  return (
    <Action
      label={done ? "Copied" : "Copy"}
      icon={done ? Check : Copy}
      onClick={() => {
        void copyText(text).then((ok) => {
          if (!ok) return;
          setDone(true);
          setTimeout(() => setDone(false), 1200);
        });
      }}
    />
  );
}

/**
 * The row under a message: when it was said, then what can be done to it.
 *
 * Its height is always taken, and only its contents fade in on hover — a row
 * that appeared would push the whole thread down a line every time the
 * pointer crossed it.
 */
function MessageActions({
  when,
  at,
  side,
  children,
}: {
  when?: string;
  /** The playhead second a dock question was asked at. Beside the wall clock
   *  rather than in the bubble: it is a fact *about* the message, the same
   *  kind of thing as when it was asked, and the bubble holds only what was
   *  typed. */
  at?: number | null;
  side: "left" | "right";
  children: React.ReactNode;
}) {
  return (
    <div
      // Furniture, not the conversation: a selection dragged across several
      // messages must not come out with a clock time between them
      // (`selectionMarkdown`).
      data-copy-skip
      className={cn(
        "-mt-0.5 flex h-8 items-center gap-1 text-[11px] text-muted-foreground opacity-0 transition-opacity focus-within:opacity-100 group-hover/msg:opacity-100",
        side === "right" ? "justify-end" : "pl-1",
      )}
    >
      {when && <span className="px-1.5 tabular-nums">{when}</span>}
      {at != null && (
        <span className="-ml-1 pr-1.5 tabular-nums" title="The moment this message carried">
          at {fmtTime(at)}
        </span>
      )}
      {children}
    </div>
  );
}

/** Grows to its content, so an edited question is never a three-line window
 *  onto a ten-line message. */
const EDIT_MAX_H = 240;

/** How much of a long question is left showing when it is folded. A pasted
 *  brief is routinely longer than the screen, and a thread of them reads as
 *  one wall of text with the answers lost inside it. */
const QUESTION_MAX_H = 208;
/** Below this much hidden, folding would save a line and cost a click. */
const FOLD_SLACK = 40;

/** Whether a bubble is long enough to be worth folding — measured, not
 *  counted: how many lines a paste becomes is the column's decision, and the
 *  column changes width with the panel. */
function useOverflows(text: string, shown: boolean) {
  const ref = useRef<HTMLDivElement>(null);
  const [over, setOver] = useState(false);
  useLayoutEffect(() => {
    const el = ref.current;
    // Nothing to measure while the bubble is a box instead; the node this
    // watches is gone, so the observer has to be re-hung when it comes back.
    if (!el || !shown) return;
    // `scrollHeight` is the full text even while `max-height` is clipping it.
    const measure = () => setOver(el.scrollHeight > QUESTION_MAX_H + FOLD_SLACK);
    measure();
    const ro = new ResizeObserver(measure);
    ro.observe(el);
    return () => ro.disconnect();
  }, [text, shown]);
  return [ref, over] as const;
}

/**
 * The pictures a question attached, lifted out of its prose, and the prose
 * with the holes they left closed up.
 *
 * Attachments go *above* the words — the shape every chat UI has settled on,
 * and the one the composer's own strip already draws. Inline they were a
 * 240px block dropped into the middle of a sentence, and five of them turned
 * a two-line question into a column of gaps.
 *
 * Closing the hole is this function's other half. The composer writes the
 * paths on their own line under the message, so removing them leaves a
 * trailing blank line `whitespace-pre-wrap` would faithfully draw; a picture
 * quoted back *inside* a sentence leaves the two spaces that were around it.
 * Both are swept here and not in `splitLibraryPaths`, which the composer's
 * own editor shares and which must hand back the text exactly as it was.
 */
function liftPictures(parts: TextPart[]): [Extract<TextPart, { kind: "image" }>[], TextPart[]] {
  const pictures = parts.filter((p) => p.kind === "image");
  if (!pictures.length) return [pictures, parts];

  // Two runs of prose that were only ever separated by a picture are one run
  // now, and the seam between them collapses: a single space inside a line,
  // nothing at all across a break.
  const body: TextPart[] = [];
  for (const p of parts) {
    if (p.kind === "image") continue;
    const last = body[body.length - 1];
    if (p.kind === "text" && last?.kind === "text") {
      const left = last.text.replace(/[ \t]+$/, "");
      const right = p.text.replace(/^[ \t]+/, "");
      const gap = !left || !right || /\n\s*$/.test(left) || /^\s*\n/.test(right) ? "" : " ";
      body[body.length - 1] = { kind: "text", text: left + gap + right };
    } else {
      body.push(p);
    }
  }

  // …and the ends, where the message's own last line used to be followed by
  // a strip of paths.
  const first = body[0];
  if (first?.kind === "text") body[0] = { kind: "text", text: first.text.replace(/^\s+/, "") };
  const last = body[body.length - 1];
  if (last?.kind === "text")
    body[body.length - 1] = { kind: "text", text: last.text.replace(/\s+$/, "") };

  return [pictures, body.filter((p) => p.kind !== "text" || p.text.length > 0)];
}

/**
 * A question: the student's own words, on the right.
 *
 * Editing happens in the bubble itself rather than back in the composer —
 * the thread is where the question is — and the box is the bubble grown to
 * the column's width, with its two buttons inside it.
 */
function QuestionBubble({
  msgId,
  text,
  pending,
  when,
  at,
  onSubmit,
  onRemove,
  onRewind,
}: {
  /** The row id, for the rail to measure. Absent on a queued message: it is
   *  not a question yet, so it is not a landmark. */
  msgId?: number;
  text: string;
  pending?: boolean;
  when?: string;
  /** The playhead second this question carried, for a dock message. */
  at?: number | null;
  /** Absent while the thread is busy — a rewind under a running turn would
   *  delete rows it is still writing. */
  onSubmit?: (text: string) => void;
  onRemove?: () => void;
  onRewind?: () => void;
}) {
  const [editing, setEditing] = useState<string | null>(null);
  const [open, setOpen] = useState(false);
  /** The picture being looked at, as the src the card already drew — opening
   *  one is a viewer over the thread, not a trip out to the OS. */
  const [shown, setShown] = useState<string | null>(null);
  // One IPC call for the whole app, so a thread of bubbles costs nothing
  // (`useDataDir`). Only a bubble holding a picture ever reads it.
  const dataDir = useDataDir();
  const [body, long] = useOverflows(text, editing === null);
  const box = useRef<HTMLTextAreaElement>(null);
  // The mentions in the question, drawn as the chips they were picked as
  // rather than as the paths the agent was sent. Split on shape, so a bubble
  // costs no query — see `splitLibraryPaths`.
  const parts = useMemo(() => splitLibraryPaths(text), [text]);
  // Attachments above, prose below — and the prose is what the fold measures
  // and can hide, because a picture the student attached is the question's
  // subject and never the part worth folding away.
  const [pictures, prose] = useMemo(() => liftPictures(parts), [parts]);

  useLayoutEffect(() => {
    const el = box.current;
    if (!el || editing === null) return;
    el.style.height = "0px";
    el.style.height = `${Math.min(el.scrollHeight, EDIT_MAX_H)}px`;
  }, [editing]);

  if (editing !== null && onSubmit) {
    const save = () => {
      const t = editing.trim();
      setEditing(null);
      if (t && t !== text) onSubmit(t);
    };
    return (
      <div className="w-full rounded-2xl border border-border bg-surface px-4 py-3">
        <Textarea
          ref={box}
          autoFocus
          value={editing}
          onChange={(e) => setEditing(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Escape") {
              e.preventDefault();
              setEditing(null);
            }
            if (e.key === "Enter" && !e.shiftKey) {
              e.preventDefault();
              save();
            }
          }}
          rows={1}
          className="max-h-60 w-full resize-none overflow-y-auto border-0 bg-transparent p-0 text-[13px]! leading-relaxed shadow-none focus-visible:border-0 focus-visible:ring-0 dark:bg-transparent"
        />
        <div className="mt-3 flex items-center justify-end gap-2">
          <Button size="xs" variant="outline" onClick={() => setEditing(null)}>
            Cancel
          </Button>
          <Button size="xs" onClick={save}>
            {pending ? "Save" : "Send"}
          </Button>
        </div>
      </div>
    );
  }

  return (
    <div className="group/msg flex w-full flex-col">
      {/* One viewer for the message, not one per card: only one picture can
          be open at a time, and a dialog per thumbnail would be a portal per
          thumbnail. */}
      <ImageLightbox
        src={shown ?? ""}
        alt="Attached picture"
        open={shown !== null}
        onOpenChange={(o) => !o && setShown(null)}
      />
      {/* The landmark the rail measures is the question as drawn — its
          pictures and its words, but not the action row under them, which
          appears on hover and would make a tick jump. */}
      <div data-msg-id={msgId} className="flex w-full flex-col items-end gap-1.5">
        {pictures.length > 0 && (
          // The pictures themselves, not chips, and **beside the bubble
          // rather than inside it**: what was attached is what the question
          // was about, a filename of digits says nothing about it, and every
          // chat UI has settled on the same shape — cards of their own above
          // the words. Clicking one opens it full size the way any other file
          // in the library opens.
          //
          // One picture draws at its own size. Several become a three-across
          // grid that wraps, each card letterboxing its picture on the
          // bubble's own ground instead of cropping it square: what a student
          // attaches here is a screenshot of a question, and a crop of one is
          // unreadable, which is the whole point of drawing it at all.
          <div className="flex max-w-[70%] flex-wrap justify-end gap-1.5">
            {pictures.map((p, i) => (
              <button
                key={i}
                type="button"
                aria-label="Open the attached picture"
                onClick={() => setShown(attachmentSrc(dataDir, p.path))}
                className={cn(
                  "cursor-pointer overflow-hidden rounded-xl border border-border bg-surface transition-colors hover:border-ring",
                  pictures.length === 1
                    ? "max-w-full"
                    : "aspect-video w-[calc((100%-0.75rem)/3)]",
                )}
              >
                <img
                  src={attachmentSrc(dataDir, p.path)}
                  alt="Attached picture"
                  className={cn(
                    "block",
                    pictures.length === 1
                      ? "max-h-72 w-auto max-w-full"
                      : "h-full w-full object-contain",
                  )}
                />
              </button>
            ))}
          </div>
        )}
        {(prose.length > 0 || pictures.length === 0) && (
          <div
            className={cn(
              "max-w-[70%] min-w-0 rounded-xl border px-3.5 py-2 text-[13px] leading-relaxed",
              pending
                ? "border-dashed border-border bg-transparent text-muted-foreground"
                : "border-border bg-surface text-foreground",
            )}
          >
            <div
              ref={body}
              // The fade is a mask rather than a gradient over the top: the
              // bubble behind it is two different grounds (queued is
              // transparent), and a mask does not need to know which.
              className={cn(
                "whitespace-pre-wrap break-words",
                long && !open && "overflow-hidden [mask-image:linear-gradient(to_bottom,#000_calc(100%-2.25rem),transparent)]",
              )}
              style={long && !open ? { maxHeight: QUESTION_MAX_H } : undefined}
            >
              {prose.map((p, i) => {
                if (p.kind === "text") return p.text;
                return (
                  <FileChip
                    key={i}
                    path={p.path}
                    onClick={(newTab) => openLibraryPath(p.path, newTab)}
                  />
                );
              })}
            </div>
            {long && (
              <button
                type="button"
                data-copy-skip
                aria-expanded={open}
                onClick={() => setOpen((o) => !o)}
                className="mt-1.5 flex cursor-pointer items-center gap-1 text-[11px] text-muted-foreground transition-colors hover:text-foreground"
              >
                {open ? "Show less" : "Show more"}
                <CaretDown size={11} className={cn("transition-transform", open && "rotate-180")} />
              </button>
            )}
          </div>
        )}
      </div>
      <MessageActions when={pending ? "Queued" : when} at={pending ? null : at} side="right">
        <CopyAction text={text} />
        {onSubmit && <Action label="Edit" icon={PencilSimple} onClick={() => setEditing(text)} />}
        {onRewind && <Action label="Rewind to here" icon={ArrowCounterClockwise} onClick={onRewind} />}
        {onRemove && <Action label="Remove" icon={X} onClick={onRemove} />}
      </MessageActions>
    </div>
  );
}

/** `data-msg-id` is what the left-hand rail measures — the questions are its
 *  landmarks (`ThreadMap.tsx`). Whether this one can be edited is read here
 *  rather than passed in: the answer changes twice a turn, and a prop would
 *  re-render every committed row with it, markdown and all. */
const User = memo(function User({
  item,
  actions,
}: {
  item: HarnessItem;
  actions?: QuestionActions;
}) {
  const busy = useHarnessStore((s) => s.live[item.thread_id]?.running ?? false);
  const text = item.content ?? "";
  const edit = busy ? undefined : actions?.edit;
  const rewind = busy ? undefined : actions?.rewind;
  return (
    <QuestionBubble
      msgId={item.id}
      text={text}
      when={fmtClock(sqliteUtcToMs(item.created_at))}
      at={messageAt(item)}
      onSubmit={edit ? (next) => edit(item.id, next) : undefined}
      onRewind={rewind ? () => rewind(item.id) : undefined}
    />
  );
});

/** An answer, and under it the two things anyone ever wants from one: the
 *  text, and the same question asked again. */
const Reply = memo(function Reply({
  item,
  asked,
  actions,
}: {
  item: HarnessItem;
  /** The question this answered, for Retry. Absent for an answer with no
   *  question above it, which only a rewound thread can produce. */
  asked?: { id: number; text: string };
  actions?: QuestionActions;
}) {
  const busy = useHarnessStore((s) => s.live[item.thread_id]?.running ?? false);
  const text = item.content ?? "";
  const retry = actions?.retry;
  return (
    <div className="group/msg flex w-full min-w-0 flex-col">
      <Assistant text={text} />
      <MessageActions when={fmtClock(sqliteUtcToMs(item.created_at))} side="left">
        <CopyAction text={text} />
        {retry && asked && !busy && (
          <Action
            label="Retry"
            icon={ArrowClockwise}
            onClick={() => retry(asked.id, asked.text)}
          />
        )}
      </MessageActions>
    </div>
  );
});

/** Where a turn was stopped. The answer above it breaks off mid-sentence,
 *  and without this the thread reads as an agent that gave up. */
const Stopped = memo(function Stopped() {
  return (
    <div className="py-1 text-center text-[11px] text-muted-foreground">You stopped the response</div>
  );
});

/** A rewind that took rows off the screen without taking them out of the
 *  agent's head — an old question with no anchor, or a session the CLI has
 *  dropped. Everywhere else the two agree, so the one case where they do not
 *  has to be visible: otherwise it surfaces as an agent referring to an
 *  answer that is not there. */
const ContextDrift = memo(function ContextDrift({ threadId }: { threadId: number }) {
  const drifted = useHarnessStore((s) => s.contextDrift[threadId] ?? false);
  if (!drifted) return null;
  return (
    <div className="py-1 text-center text-[11px] text-muted-foreground">
      The agent still remembers what was removed here
    </div>
  );
});

/** A tool call still running is the one committed row that changes: its
 *  output arrives after it. It takes that from the store itself, so the
 *  stream re-renders this row and nothing around it. */
function Tool({ item, dim }: { item: HarnessItem; dim: boolean }) {
  const done = parseToolMeta(item).ok != null;
  const ref = item.ref_id;
  const liveOutput = useHarnessStore((s) =>
    done || !ref ? undefined : s.live[item.thread_id]?.toolOutput[ref],
  );
  return <ToolRow item={item} dim={dim} liveOutput={liveOutput} />;
}

const Item = memo(function Item({
  item,
  dim,
  actions,
  asked,
  onSignIn,
  latest,
}: {
  item: HarnessItem;
  dim: boolean;
  /** Stable for the life of the page, so memoising these rows still works. */
  actions?: QuestionActions;
  /** For an answer: the question above it. Memoised with `items`, so the
   *  object identity is as stable as the rows are. */
  asked?: { id: number; text: string };
  /** Opens the sign-in dialog for a signed-out agent. It is `setState` from
   *  the `Timeline` below, so its identity is stable on the same terms
   *  `actions` is — the dialog has to outlive this row re-rendering, and a
   *  fresh closure per render would un-memoise every row in the thread. */
  onSignIn?: (provider: Provider) => void;
  /** For a `permission` row: whether it is the thread's latest refusal, the
   *  one that still carries the allow button. */
  latest?: boolean;
}) {
  switch (item.kind) {
    case "user":
      return <User item={item} actions={actions} />;
    case "assistant":
      return <Reply item={item} asked={asked} actions={actions} />;
    case "thinking":
      return <ThinkingRow text={item.content ?? ""} dim={dim} />;
    case "tool":
      return <Tool item={item} dim={dim} />;
    case "error":
      return (
        <ErrorRow text={item.content ?? ""} auth={parseErrorMeta(item).auth} onSignIn={onSignIn} />
      );
    case "interrupted":
      return <Stopped />;
    case "permission":
      return <PermissionCard item={item} actionable={!!latest} onFollowUp={actions?.followUp} />;
  }
});

const Bundle = memo(function Bundle({ items }: { items: HarnessItem[] }) {
  const { label, icon } = useMemo(() => bundleLabel(items), [items]);
  return (
    <RowShell icon={TOOL_ICON[icon]} title={label} expandable dim>
      {/* The group line: rows hang off a hairline, the way nested work does in bb. */}
      <div className="relative my-0.5 pl-3 before:absolute before:bottom-1 before:left-1.5 before:top-0 before:w-px before:bg-border before:content-['']">
        <div className="flex flex-col">
          {items.map((i) => (
            <Item key={i.id} item={i} dim={false} />
          ))}
        </div>
      </div>
    </RowShell>
  );
});

/** The turn in flight: reasoning and text that have no row yet, and the
 *  "Working…" line for the beat when neither is arriving. The only part of
 *  the timeline subscribed to the stream. */
function LiveTail({ threadId }: { threadId: number }) {
  const live = useHarnessStore((s) => s.live[threadId]);
  if (!live) return null;
  const quiet = live.running && !live.streaming && !live.thinking;
  return (
    <>
      {live.thinking && <ThinkingRow text={live.thinking} live />}
      {live.streaming && <Assistant text={live.streaming} />}
      {quiet && (
        <div className="mt-1 flex items-center gap-2 px-2 text-xs text-muted-foreground">
          <CircleNotch size={13} className="animate-spin" />
          <span className="animate-pulse">Working…</span>
        </div>
      )}
    </>
  );
}

/**
 * What was typed while the agent was working, waiting its turn.
 *
 * These are not rows and not history: Rust is holding them in memory and will
 * send them one at a time as the turn in front of each one ends
 * (`Queue` in `app/src-tauri/src/harness/mod.rs`). Until then they can be
 * rewritten or dropped, which is the whole reason they are drawn as bubbles
 * here rather than left invisible in the composer.
 */
function Pending({ threadId, actions }: { threadId: number; actions: PendingActions }) {
  const queued = useHarnessStore((s) => s.queued[threadId]);
  if (!queued?.length) return null;
  return (
    <>
      {queued.map((q) => (
        <QuestionBubble
          key={q.id}
          text={q.text}
          pending
          onSubmit={(text) => actions.editQueued(q.id, text)}
          onRemove={() => actions.unqueue(q.id)}
        />
      ))}
    </>
  );
}

/**
 * Copying out of a thread gives **markdown**, not the words as they are set.
 *
 * What the browser would put on the clipboard is the rendering: a heading
 * without its `#`, a table as a run of words, a formula as KaTeX's glyphs. The
 * Copy button under a message has the source string and hands that over; a
 * selection dragged across half an answer has no source string, so the DOM
 * inside it is read back into markdown instead (`lib/selectionMarkdown.ts`).
 *
 * Only `text/plain` is written, which is the flavour a notes app, an editor
 * and another agent all take. Plain text is a later choice — the right-click
 * menu this app does not have yet is where it belongs.
 */
function markdownFor(target: EventTarget | null): string {
  // A selection inside a field belongs to the field, and it is already text.
  if (target instanceof Element && target.closest("input, textarea, [contenteditable='true']")) {
    return "";
  }
  return selectionMarkdown(window.getSelection());
}

function copyAsMarkdown(e: React.ClipboardEvent) {
  const md = markdownFor(e.target);
  if (!md) return;
  e.clipboardData.setData("text/plain", md);
  // Without this the browser writes its own flavours over ours.
  e.preventDefault();
}

/** The same text, dragged out instead of copied. **No `preventDefault` here**:
 *  on `dragstart` that cancels the drag outright (CLAUDE.md) — `setData` alone
 *  replaces what WebKit had already put on the transfer. */
function dragAsMarkdown(e: React.DragEvent) {
  const md = markdownFor(e.target);
  if (md) e.dataTransfer.setData("text/plain", md);
}

export interface PendingActions {
  editQueued: (queueId: string, text: string) => void;
  unqueue: (queueId: string) => void;
}

export function Timeline({
  items,
  threadId,
  running,
  questions,
  pending,
}: {
  items: HarnessItem[];
  threadId: number | null;
  running: boolean;
  /** Stable for the life of the page — see `Item`. */
  questions?: QuestionActions;
  pending?: PendingActions;
}) {
  /**
   * Which agent's sign-in dialog is open, if any.
   *
   * Here rather than inside the row that offers it, for the reason the install
   * run sits in its Settings section: the dialog and the flow behind it have
   * to survive the row re-rendering — and a thread re-renders constantly, both
   * ends of every turn. `setSignIn` is React's own setter, so handing it
   * straight to a memoised `Item` costs that memoisation nothing.
   *
   * It lives in `Timeline` rather than in `ChatPage` because the timeline is
   * the surface that is in two places: the chat page and the lecture player's
   * dock. A failed turn shows the same card in both, so the dialog has to be
   * in both too.
   */
  const [signIn, setSignIn] = useState<Provider | null>(null);
  const { recheck } = useSignInStatus();
  const run = useSignIn(recheck);

  const rows = useMemo(() => buildRows(items, running), [items, running]);
  // Which question each answer answered — what Retry asks again. Built with
  // the rows so the object handed to a memoised row keeps its identity.
  const asked = useMemo(() => {
    const map = new Map<number, { id: number; text: string }>();
    let last: { id: number; text: string } | undefined;
    for (const i of items) {
      if (i.kind === "user") last = { id: i.id, text: i.content ?? "" };
      else if (i.kind === "assistant" && last) map.set(i.id, last);
    }
    return map;
  }, [items]);
  // Only the newest refusal is still a question; the ones above it are what
  // happened. Allowing is idempotent, so this is tidiness, not safety.
  const latestPermission = useMemo(() => {
    for (let i = items.length - 1; i >= 0; i--) if (items[i].kind === "permission") return items[i].id;
    return undefined;
  }, [items]);
  return (
    <div
      className="flex min-w-0 flex-col gap-2"
      onCopy={copyAsMarkdown}
      onDragStart={dragAsMarkdown}
    >
      {rows.map((r) =>
        r.kind === "bundle" ? (
          <Bundle key={r.id} items={r.items} />
        ) : (
          <Item
            key={r.item.id}
            item={r.item}
            dim={r.dim}
            actions={questions}
            asked={asked.get(r.item.id)}
            onSignIn={setSignIn}
            latest={r.item.id === latestPermission}
          />
        ),
      )}
      {threadId != null && <ContextDrift threadId={threadId} />}
      {threadId != null && <LiveTail threadId={threadId} />}
      {threadId != null && pending && <Pending threadId={threadId} actions={pending} />}
      {signIn && (
        <SignInDialog
          provider={signIn}
          run={run.run?.provider === signIn ? run.run : null}
          onStart={() => run.start(signIn)}
          onCode={(code) => run.submitCode(code)}
          onCancel={() => run.cancel()}
          onClose={() => {
            // A finished run is cleared with the dialog, so reopening the card
            // offers the sign-in again rather than a log of what already
            // happened. One still in flight is kept — the browser is still
            // open on it — and reopening resumes the same run.
            if (run.run?.result) run.clear();
            setSignIn(null);
          }}
        />
      )}
    </div>
  );
}
