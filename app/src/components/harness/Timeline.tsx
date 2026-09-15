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
import { MD_COMPONENTS } from "@/components/markdown/MdComponents";
import { Button } from "@/components/ui/button";
import { Textarea } from "@/components/ui/textarea";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { fmtClock, sqliteUtcToMs } from "@/lib/format";
import { fmtTime } from "@/lib/lectures";
import { messageAt, parseToolMeta, type HarnessItem, type ToolKind } from "@/lib/harness";
import { useHarnessStore } from "@/stores/harnessStore";
import { cn, copyText } from "@/lib/utils";
import { ErrorRow, RowShell, ThinkingRow, ToolRow, TOOL_ICON } from "./WorkRow";

/** Editing a question, or one still waiting to be asked, both happen in the
 *  bubble itself rather than back in the composer: the thread is where the
 *  question is, and a box that opened somewhere else would lose its place in
 *  the conversation it is being asked about. */
export interface QuestionActions {
  /** Ask it again, differently. The thread rewinds to this row. */
  edit: (itemId: number, text: string) => void;
  /** Take the thread back to just before this question and hand its words to
   *  the composer. Claude Code's rewind, without the branching. */
  rewind: (itemId: number) => void;
  /** Ask the same question again, unchanged — what Retry under an answer is. */
  retry: (itemId: number, text: string) => void;
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

/** KaTeX is the most expensive thing in the pipeline and most replies have no
 *  maths in them at all, so the two math plugins are only loaded over text
 *  that carries a delimiter. */
const MATH = /\$|\\\(|\\\[/;
/** One frozen empty array, so the no-maths path keeps prop identity too. */
const NO_PLUGINS: never[] = [];

/** `chat-md` scales the shared markdown components down to chat's own size —
 *  see the rule in `app/src/index.css`. The components themselves are sized
 *  for a document (the file viewer), which is a size too large for a reply. */
const Assistant = memo(function Assistant({ text }: { text: string }) {
  const math = MATH.test(text);
  return (
    <div className="chat-md min-w-0 px-2 text-[13px] leading-relaxed">
      <ReactMarkdown
        remarkPlugins={math ? [remarkGfm, remarkMath] : [remarkGfm]}
        rehypePlugins={math ? [rehypeKatex] : NO_PLUGINS}
        components={MD_COMPONENTS}
      >
        {text}
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
  const [body, long] = useOverflows(text, editing === null);
  const box = useRef<HTMLTextAreaElement>(null);

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
      <div data-msg-id={msgId} className="flex w-full justify-end">
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
            {text}
          </div>
          {long && (
            <button
              type="button"
              aria-expanded={open}
              onClick={() => setOpen((o) => !o)}
              className="mt-1.5 flex cursor-pointer items-center gap-1 text-[11px] text-muted-foreground transition-colors hover:text-foreground"
            >
              {open ? "Show less" : "Show more"}
              <CaretDown size={11} className={cn("transition-transform", open && "rotate-180")} />
            </button>
          )}
        </div>
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
  const live = actions && !busy;
  return (
    <QuestionBubble
      msgId={item.id}
      text={text}
      when={fmtClock(sqliteUtcToMs(item.created_at))}
      at={messageAt(item)}
      onSubmit={live ? (next) => actions.edit(item.id, next) : undefined}
      onRewind={live ? () => actions.rewind(item.id) : undefined}
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
  return (
    <div className="group/msg flex w-full min-w-0 flex-col">
      <Assistant text={text} />
      <MessageActions when={fmtClock(sqliteUtcToMs(item.created_at))} side="left">
        <CopyAction text={text} />
        {actions && asked && !busy && (
          <Action
            label="Retry"
            icon={ArrowClockwise}
            onClick={() => actions.retry(asked.id, asked.text)}
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
}: {
  item: HarnessItem;
  dim: boolean;
  /** Stable for the life of the page, so memoising these rows still works. */
  actions?: QuestionActions;
  /** For an answer: the question above it. Memoised with `items`, so the
   *  object identity is as stable as the rows are. */
  asked?: { id: number; text: string };
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
      return <ErrorRow text={item.content ?? ""} />;
    case "interrupted":
      return <Stopped />;
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
  return (
    <div className="flex min-w-0 flex-col gap-2">
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
          />
        ),
      )}
      {threadId != null && <ContextDrift threadId={threadId} />}
      {threadId != null && <LiveTail threadId={threadId} />}
      {threadId != null && pending && <Pending threadId={threadId} actions={pending} />}
    </div>
  );
}
