import { memo, useLayoutEffect, useMemo, useRef, useState } from "react";
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
import { CompactMd } from "@/components/markdown/MdComponents";
import { FileChip } from "@/components/markdown/FileChip";
import { openCitation, splitLibraryPaths, type TextPart } from "@/lib/openFile";
import { attachmentSrc } from "@/lib/attachments";
import { ImageLightbox } from "@/components/ui/Lightbox";
import { copyAsMarkdown, dragAsMarkdown } from "@/lib/selectionMarkdown";
import { useDataDir } from "@/hooks/useDataDir";
import { Button } from "@/components/ui/button";
import { Textarea } from "@/components/ui/textarea";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { fmtClock, sqliteUtcToMs } from "@/lib/format";
import { fmtClockSecs } from "@/lib/lectures";
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

/** Edit, rewind and retry truncate the thread, so they are absent for a
 *  provider that cannot take a question back out of its own context
 *  (`ProviderInfo.rewind`). */
export interface QuestionActions {
  /** Ask it again, differently. The thread rewinds to this row. */
  edit?: (itemId: number, text: string) => void;
  /** Take the thread back to just before this question and hand its words to
   *  the composer. */
  rewind?: (itemId: number) => void;
  /** Ask the same question again, unchanged. */
  retry?: (itemId: number, text: string) => void;
  /** Send a new message on the open thread — what an approved permission
   *  carries on with. */
  followUp: (text: string) => Promise<void>;
}

/**
 * The thread as a list. Messages are the spine; the tool calls and reasoning
 * between them are a *step*, and a finished step of two or more rows folds
 * into one summary row ("Explored 3 files, ran 2 commands"). The step in
 * progress stays unfolded; finished rows are dimmed.
 *
 * **Nothing here re-renders for the turn in flight.** Committed rows never
 * change, so all are memoised; only `LiveTail` and a running `Tool` subscribe
 * to the stream.
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
  // The trailing step is live while the turn runs.
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

// Memoised: a committed reply never changes, and re-parsing it per streamed
// token is what makes the thread jitter.
const Assistant = memo(function Assistant({ text }: { text: string }) {
  return <CompactMd text={text} className="px-2 text-[13px] leading-relaxed" />;
});

/** One action under a message: an icon with a tooltip. */
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

/** The icon becomes a tick on copy (no toasts in this app). */
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

/** The row under a message: its time, then its actions. Its height is always
 *  taken and only the contents fade in on hover, so the thread never jumps. */
function MessageActions({
  when,
  at,
  side,
  children,
}: {
  when?: string;
  /** The playhead second a dock question was asked at. */
  at?: number | null;
  side: "left" | "right";
  children: React.ReactNode;
}) {
  return (
    <div
      // Kept out of a copied selection (`selectionMarkdown`), and not
      // selectable at all: a drag across it highlights the icons, which reads
      // as them bolding and unbolding.
      data-copy-skip
      className={cn(
        "-mt-0.5 flex select-none h-8 items-center gap-1 text-[11px] text-muted-foreground opacity-0 transition-opacity focus-within:opacity-100 group-hover/msg:opacity-100",
        side === "right" ? "justify-end" : "pl-1",
      )}
    >
      {when && <span className="px-1.5 tabular-nums">{when}</span>}
      {at != null && (
        <span className="-ml-1 pr-1.5 tabular-nums" title="The moment this message carried">
          at {fmtClockSecs(at)}
        </span>
      )}
      {children}
    </div>
  );
}

/** The edit box grows to its content up to this. */
const EDIT_MAX_H = 240;

/** How much of a long question is left showing when it is folded. */
const QUESTION_MAX_H = 208;
/** Below this much hidden, folding would save a line and cost a click. */
const FOLD_SLACK = 40;

/** Whether a bubble is long enough to fold — measured, since line count
 *  depends on the column's width. */
function useOverflows(text: string, shown: boolean) {
  const ref = useRef<HTMLDivElement>(null);
  const [over, setOver] = useState(false);
  useLayoutEffect(() => {
    const el = ref.current;
    // While editing the node is gone; the observer re-hangs when it returns.
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
 * A question's attached pictures, lifted out to draw above the words, and the
 * prose with the gaps they left closed up. Swept here, not in
 * `splitLibraryPaths`, which the composer shares and must round-trip exactly.
 */
function liftPictures(parts: TextPart[]): [Extract<TextPart, { kind: "image" }>[], TextPart[]] {
  const pictures = parts.filter((p) => p.kind === "image");
  if (!pictures.length) return [pictures, parts];

  // Prose split only by a picture joins with one space, or none across a break.
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

  // The composer writes attachment paths on their own trailing line.
  const first = body[0];
  if (first?.kind === "text") body[0] = { kind: "text", text: first.text.replace(/^\s+/, "") };
  const last = body[body.length - 1];
  if (last?.kind === "text")
    body[body.length - 1] = { kind: "text", text: last.text.replace(/\s+$/, "") };

  return [pictures, body.filter((p) => p.kind !== "text" || p.text.length > 0)];
}

/** A question, on the right. Editing (a sent or a queued one) happens in the
 *  bubble itself, grown to the column's width. */
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
  /** The row id, for the rail to measure. Absent on a queued message. */
  msgId?: number;
  text: string;
  pending?: boolean;
  when?: string;
  /** The playhead second, for a dock message. */
  at?: number | null;
  /** Absent while the thread is busy: a rewind would delete rows mid-write. */
  onSubmit?: (text: string) => void;
  onRemove?: () => void;
  onRewind?: () => void;
}) {
  const [editing, setEditing] = useState<string | null>(null);
  const [open, setOpen] = useState(false);
  /** The picture open in the lightbox, as the src the card drew. */
  const [shown, setShown] = useState<string | null>(null);
  const dataDir = useDataDir();
  const [body, long] = useOverflows(text, editing === null);
  const box = useRef<HTMLTextAreaElement>(null);
  // Mentions draw as chips rather than the paths the agent was sent.
  const parts = useMemo(() => splitLibraryPaths(text), [text]);
  // Only the prose is measured and folded; pictures always show.
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
      {/* One viewer per message, not per thumbnail. */}
      <ImageLightbox
        src={shown ?? ""}
        alt="Attached picture"
        open={shown !== null}
        onOpenChange={(o) => !o && setShown(null)}
      />
      {/* What the rail measures: pictures and words, not the action row. */}
      <div data-msg-id={msgId} className="flex w-full flex-col items-end gap-1.5">
        {pictures.length > 0 && (
          // One picture at its own size; several in a three-across grid,
          // letterboxed rather than cropped — they are usually screenshots.
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
              // A mask, not a gradient: a queued bubble's ground is transparent.
              className={cn(
                "whitespace-pre-wrap break-words",
                long && !open && "overflow-hidden [mask-image:linear-gradient(to_bottom,#000_calc(100%-2.25rem),transparent)]",
              )}
              style={long && !open ? { maxHeight: QUESTION_MAX_H } : undefined}
            >
              {prose.map((p, i) => {
                if (p.kind === "text") return p.text;
                if (p.kind !== "path") return null;
                return (
                  <FileChip
                    key={i}
                    path={p.path}
                    cite={p.cite}
                    onClick={(newTab) => openCitation(p.cite, newTab)}
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

/** Busy is read from the store here, not passed in: it changes twice a turn,
 *  and a prop would re-render every committed row. */
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

/** An answer, with Copy and Retry under it. */
const Reply = memo(function Reply({
  item,
  asked,
  actions,
}: {
  item: HarnessItem;
  /** The question this answered, for Retry. */
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

/** Where a turn was stopped. */
const Stopped = memo(function Stopped() {
  return (
    <div className="py-1 text-center text-[11px] text-muted-foreground">You stopped the response</div>
  );
});

/** A rewind that took rows off the screen but not out of the agent's context
 *  (no anchor, or a dropped CLI session) — shown so the mismatch is visible. */
const ContextDrift = memo(function ContextDrift({ threadId }: { threadId: number }) {
  const drifted = useHarnessStore((s) => s.contextDrift[threadId] ?? false);
  if (!drifted) return null;
  return (
    <div className="py-1 text-center text-[11px] text-muted-foreground">
      The agent still remembers what was removed here
    </div>
  );
});

/** A running tool's output streams in; it subscribes itself so only this row
 *  re-renders. */
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
  /** For an answer: the question above it (identity-stable with `items`). */
  asked?: { id: number; text: string };
  /** Opens the sign-in dialog; a `setState`, so stable. */
  onSignIn?: (provider: Provider) => void;
  /** For a `permission` row: the latest refusal carries the allow button. */
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

/** The turn in flight: reasoning and text with no row yet, or "Working…". */
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

/** Messages queued while the agent works — held in memory by Rust (`Queue` in
 *  `app/src-tauri/src/harness/mod.rs`), editable or removable until sent. */
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
  // Held here, not in the row, so the dialog survives rows re-rendering; and
  // in `Timeline`, not `ChatPage`, so the lecture dock gets it too.
  const [signIn, setSignIn] = useState<Provider | null>(null);
  const { recheck } = useSignInStatus();
  const run = useSignIn(recheck);

  const rows = useMemo(() => buildRows(items, running), [items, running]);
  // Which question each answer answered, for Retry.
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
            // Clear a finished run; keep one in flight so reopening resumes it.
            if (run.run?.result) run.clear();
            setSignIn(null);
          }}
        />
      )}
    </div>
  );
}
