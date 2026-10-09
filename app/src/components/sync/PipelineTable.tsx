import { memo, useCallback, useMemo, useState, type CSSProperties, type ReactNode } from "react";
import { CaretRight, Play, SidebarSimple, SkipForward } from "@phosphor-icons/react";
import { cn } from "@/lib/utils";
import { Button } from "@/components/ui/button";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import { usePagedRows } from "@/components/ui/TablePagination";
import { GridTable } from "@/components/ui/GridTable";
import { displayCode, fmtAgo, fmtClock } from "@/lib/format";
import { fileIconFor, isSheetFile } from "@/lib/fileTypes";
import { getFileByRelativePath } from "@/lib/db";
import { filePagePath, openFileSmart } from "@/lib/openFile";
import { openBeside } from "@/lib/tabRouters";
import { summaryOf } from "@/lib/parseState";
import { useNow } from "@/hooks/useNow";
import {
  embedsIn,
  fmtEta,
  fmtMb,
  statusOf,
  uploadEta,
  usePipelineStore,
  type PipelineItem,
  type PipelinePhase,
  type StageState,
  type StatusView,
} from "@/stores/pipelineStore";

/**
 * The ingest ledger: one row per PDF through Download → Parse → Embed, live
 * work ranked first. A row is the file, one segmented track — itself the
 * status — with a caption saying what is happening, and when the file last
 * moved; actions take the time's place on hover. It expands into the facts the row
 * leaves out. With no Voyage key the embed segment isn't drawn (`embedStage`
 * in `pipelineStore`, set from `indexStore`), so a parsed file is done at two;
 * a spreadsheet is never embedded, so its row is two stages, its parse being
 * the conversion to text (`embedsIn`).
 */

type StageKey = "download" | "parse" | "embed";

const STAGE_LABEL: Record<StageKey, string> = {
  download: "Download",
  parse: "Parse",
  embed: "Embed",
};

function stageKeys(embedStage: boolean): StageKey[] {
  return embedStage ? ["download", "parse", "embed"] : ["download", "parse"];
}

/** Shared by header and rows. The time column also holds the hover actions
 *  (three `icon-xs` buttons). Sized by the table's own width (`@container`):
 *  narrow, the caption folds into the track's tooltip and the name keeps the
 *  room. */
const COLS =
  "grid grid-cols-[minmax(0,1fr)_minmax(180px,260px)_80px] @max-2xl:grid-cols-[minmax(0,1fr)_96px_80px] items-center gap-4 @max-2xl:gap-3 px-5";

/** Diagonal stripes: a stage the user skipped, distinct from one not reached. */
const HATCH: CSSProperties = {
  backgroundImage:
    "repeating-linear-gradient(-45deg, color-mix(in srgb, var(--color-muted-foreground) 45%, transparent) 0 1.5px, transparent 1.5px 4px)",
};

// ── Derived row facts ─────────────────────────────────────────────────────────

/** An active embed held by a rate limit; its countdown ticks by the second. */
function isRateLimited(item: PipelineItem): boolean {
  return item.embed === "active" && item.embedWaitingUntil != null;
}

/** Parse is in its batch but another file is uploading: not moving itself. */
function uploadWaiting(item: PipelineItem): boolean {
  return item.parse === "active" && item.parsePhase === "upload_wait";
}

/** When the file last moved: the latest stage that finished, or was skipped. */
function latestStageAt(item: PipelineItem): number {
  return Math.max(
    item.downloadedAt ?? 0,
    item.uploadedAt ?? 0,
    item.parsedAt ?? 0,
    item.embeddedAt ?? 0,
    item.skippedAt ?? 0,
  );
}

/** The stage whose failure the row's error belongs to. */
function failedStage(item: PipelineItem): StageKey {
  if (item.download === "error") return "download";
  if (item.parse === "error") return "parse";
  return "embed";
}

interface RowActions {
  /** ▶: resume, retry, embed or parse a skipped file. */
  run?: { label: string; hint: string };
  skip: boolean;
}

function actionsOf(
  item: PipelineItem,
  s: StatusView,
  embedStage: boolean,
  canRun: boolean,
  canSkip: boolean,
): RowActions {
  // A parsed-but-unembedded file gets ▶ too: the backlog isn't swept
  // automatically, so this embeds one file without committing the library.
  const embedNow = embedStage && item.parse === "done" && item.embed === "pending";
  // No Retry where it can't help: a failed download isn't on disk (the next
  // sync fetches it), a non-retryable error never clears, and a latching one
  // holds every file until its cause is fixed.
  const retryable =
    s.phase === "failed" &&
    item.download !== "error" &&
    item.errorRetryable !== false &&
    !item.errorLatching;
  let run: RowActions["run"];
  // Already queued: the embed queue dedups, so ▶ would do nothing.
  if (canRun && item.embed !== "queued") {
    if (s.phase === "skipped") run = { label: "Parse now", hint: "Parse this file now" };
    else if (retryable) run = { label: "Retry", hint: "Try this file again" };
    else if (s.phase === "paused") run = { label: "Resume", hint: "Resume where it left off" };
    else if (embedNow) run = { label: "Embed", hint: "Embed this file now" };
  }
  // Anything short of a finished parse, once the bytes are on disk. A
  // spreadsheet's conversion takes moments and bills nothing, so it is not
  // skipped.
  const skip =
    canSkip &&
    !isSheetFile(item.filename) &&
    item.download === "done" &&
    (item.parse === "pending" ||
      item.parse === "queued" ||
      item.parse === "active" ||
      item.parse === "error");
  return { run, skip };
}

const SKIP_HINT = "Skip — stops this file's parse. You can parse it later.";

/** Opens the file beside the page; through its row when there is one, so
 *  the visit is recorded like any file row's. */
function openItem(item: PipelineItem) {
  const beside = () => openBeside(filePagePath(item.subjectId, item.relativePath));
  getFileByRelativePath(item.relativePath)
    .then((file) => (file ? openFileSmart(file) : beside()))
    .catch(beside);
}

// ── Progress track ────────────────────────────────────────────────────────────

/** How a segment draws: the stage's state, with the live stage's percent. */
function Segment({
  state,
  percent,
  held,
  paused,
}: {
  state: StageState;
  /** Only for the moving stage; null draws an indeterminate pulse. */
  percent: number | null;
  /** In flight but not moving (waiting for its upload turn, rate-limited). */
  held: boolean;
  /** The stage a paused file stops at. */
  paused: boolean;
}) {
  let fill: ReactNode = null;
  let track = "bg-secondary";
  if (paused) track = "bg-warning/50";
  else if (state === "done") fill = <span className="absolute inset-0 bg-success/80" />;
  else if (state === "error") fill = <span className="absolute inset-0 bg-destructive" />;
  else if (state === "queued") track = "bg-brand/20";
  else if (state === "active" && percent != null) {
    track = "bg-brand/20";
    fill = (
      <span
        className={cn(
          "absolute inset-y-0 left-0 transition-[width] duration-500",
          held ? "bg-warning/70" : "bg-brand",
        )}
        style={{ width: `${Math.max(4, Math.min(100, percent))}%` }}
      />
    );
  } else if (state === "active") {
    track = held ? "bg-brand/20" : "bg-brand/40 animate-pulse will-change-[opacity]";
  }
  return (
    <span
      className={cn("relative block h-1.5 flex-1 overflow-hidden rounded-full", track)}
      style={state === "skipped" ? HATCH : undefined}
    >
      {fill}
    </span>
  );
}

/** The row's status: one segment per stage. Its tooltip is the whole
 *  caption, which the row drops when the table is narrow. */
function Track({
  item,
  s,
  embedStage,
}: {
  item: PipelineItem;
  s: StatusView;
  embedStage: boolean;
}) {
  const held = uploadWaiting(item) || isRateLimited(item);
  const keys = stageKeys(embedStage);
  const pausedAt = s.phase === "paused" ? keys.find((k) => item[k] !== "done") : undefined;
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        {/* The padding is the hover target; the bar itself is 6px. */}
        <div className="flex w-24 shrink-0 items-center gap-[3px] py-1.5">
          {keys.map((key) => {
            const state = item[key];
            const moving = state === "active" && s.phase === "active";
            return (
              <Segment
                key={key}
                state={state}
                percent={moving ? s.percent : null}
                held={moving && held}
                paused={key === pausedAt}
              />
            );
          })}
        </div>
      </TooltipTrigger>
      <TooltipContent>{hintOf(item, s, embedStage)}</TooltipContent>
    </Tooltip>
  );
}

/** What is happening, in one line: `statusOf`'s label, with an upload's
 *  time left or a failure's short cause. */
function captionOf(item: PipelineItem, s: StatusView): string {
  if (s.phase === "failed") {
    const stage = failedStage(item);
    if (stage === "download") return "Download failed — the next sync retries it";
    // The failure vocabulary is MinerU's; a spreadsheet's sentence is in the detail.
    if (stage === "parse" && isSheetFile(item.filename)) return "Conversion failed";
    const why = summaryOf(item.errorKind).replace(/\.$/, "");
    return `${STAGE_LABEL[stage]} failed${why ? ` — ${why}` : ""}`;
  }
  const eta = uploadEta(item);
  return eta != null ? `${s.label} · ${fmtEta(eta)}` : s.label;
}

/** The track's tooltip: the caption, with a finished file's page count. */
function hintOf(item: PipelineItem, s: StatusView, embedStage: boolean): string {
  if (s.phase !== "done") return captionOf(item, s);
  const pages = embedStage && item.embed === "done" ? item.embedTotalPages : item.totalPages;
  return pages > 0 ? `${s.label} · ${pages} page${pages === 1 ? "" : "s"}` : s.label;
}

/** A held embed's caption counts down by the second; only this row ticks. */
function RateLimitedCaption({ item, embedStage }: { item: PipelineItem; embedStage: boolean }) {
  const now = useNow(1_000).getTime();
  return <CaptionText text={statusOf(item, embedStage, now).label} />;
}

function CaptionText({ text, tone }: { text: string; tone?: "bad" }) {
  return (
    <span
      className={cn(
        "min-w-0 truncate text-[11px] tabular-nums @max-2xl:hidden",
        tone === "bad" ? "text-destructive" : "text-muted-foreground",
      )}
    >
      {text}
    </span>
  );
}

// ── Actions ───────────────────────────────────────────────────────────────────

function IconAction({
  hint,
  onClick,
  children,
}: {
  hint: string;
  onClick: () => void;
  children: ReactNode;
}) {
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <Button
          variant="ghost"
          size="icon-xs"
          aria-label={hint}
          data-tab-skip
          onClick={(e) => {
            e.stopPropagation();
            onClick();
          }}
          className="text-muted-foreground hover:text-foreground"
        >
          {children}
        </Button>
      </TooltipTrigger>
      <TooltipContent>{hint}</TooltipContent>
    </Tooltip>
  );
}

// ── Expanded detail ───────────────────────────────────────────────────────────

const DOT: Record<StageState, string> = {
  pending: "bg-muted-foreground/25",
  queued: "bg-brand/40",
  active: "bg-brand",
  done: "bg-success",
  error: "bg-destructive",
  skipped: "bg-muted-foreground/50",
};

interface Step {
  key: string;
  label: string;
  state: StageState;
  /** Only what the collapsed row doesn't already say. */
  value?: string;
}

/** The upload as its own step, for a cloud parse that reported one. */
function uploadStep(item: PipelineItem): Step | null {
  if (item.bytesTotal == null) return null;
  const size = `${fmtMb(item.bytesTotal)} MB`;
  if (item.parse === "active" && item.parsePhase === "upload_wait") {
    return { key: "upload", label: "Upload", state: "queued", value: `${size}, waiting its turn` };
  }
  if (item.parse === "active" && item.parsePhase === "uploading") {
    const since = item.uploadFirstAt ? `started ${fmtClock(item.uploadFirstAt)}` : undefined;
    return { key: "upload", label: "Uploading", state: "active", value: since };
  }
  if (item.uploadedAt != null || item.parse === "done") {
    const at = fmtClock(item.uploadedAt);
    return { key: "upload", label: "Uploaded", state: "done", value: at ? `${size} · ${at}` : size };
  }
  return { key: "upload", label: "Upload", state: "pending" };
}

function detailSteps(item: PipelineItem, embedStage: boolean): Step[] {
  const steps: Step[] = [];
  const d = item.download;
  steps.push({
    key: "download",
    label: d === "done" ? "Downloaded" : d === "active" ? "Downloading" : d === "error" ? "Download failed" : "Download",
    state: d,
    value: d === "done" ? fmtClock(item.downloadedAt) || undefined : undefined,
  });

  const upload = uploadStep(item);
  if (upload) steps.push(upload);

  const p = item.parse;
  // While its upload runs, the parse itself has not started.
  const uploadingNow = p === "active" && item.parsePhase !== undefined && item.parsePhase !== "processing";
  const parseState: StageState = uploadingNow ? "pending" : p;
  const sheet = isSheetFile(item.filename);
  const parseLabel =
    parseState === "done"
      ? sheet ? "Converted" : "Parsed"
      : parseState === "active"
        ? sheet ? "Converting" : "Parsing"
        : parseState === "error"
          ? sheet ? "Conversion failed" : "Parse failed"
          : parseState === "skipped"
            ? "Skipped"
            : sheet ? "Convert" : "Parse";
  const parseValue =
    parseState === "done"
      ? fmtClock(item.parsedAt) || undefined
      : parseState === "active"
        ? item.uploadedAt
          ? `started ${fmtClock(item.uploadedAt)}`
          : undefined
        : parseState === "skipped"
          ? fmtClock(item.skippedAt) || undefined
          : undefined;
  steps.push({ key: "parse", label: parseLabel, state: parseState, value: parseValue });

  if (embedStage) {
    const e = item.embed;
    steps.push({
      key: "embed",
      label: e === "done" ? "Embedded" : e === "active" ? "Embedding" : e === "error" ? "Embed failed" : "Embed",
      state: e,
      value: e === "done" ? fmtClock(item.embeddedAt) || undefined : undefined,
    });
  }
  return steps;
}

/** One plain sentence about why the row is where it is, when that isn't
 *  obvious from the row: an error's whole message, or what a wait means. */
function noteOf(item: PipelineItem): { text: string; bad: boolean } | null {
  if (item.download === "error") {
    return { text: `${item.error ?? "Download failed"}. The next sync downloads it again.`, bad: true };
  }
  if (item.parse === "error" || item.embed === "error") {
    const text = item.errorLatching
      ? `${item.error ?? "Failed"} — this holds every file until it is fixed.`
      : (item.error ?? "Failed");
    return { text, bad: true };
  }
  if (uploadWaiting(item)) {
    return {
      text: "Another file in the same upload batch is still uploading; this one goes up after it.",
      bad: false,
    };
  }
  if (item.parse === "skipped") {
    return { text: "It stays without Markdown, search or embeddings until you parse it.", bad: false };
  }
  return null;
}

function Detail({
  item,
  embedStage,
  actions,
  onRun,
  onSkip,
}: {
  item: PipelineItem;
  embedStage: boolean;
  actions: RowActions;
  onRun?: () => void;
  onSkip?: () => void;
}) {
  const steps = detailSteps(item, embedStage);
  const note = noteOf(item);
  return (
    // Indented to the filename: px-5, the caret, the file icon and their gaps.
    <div className="pb-3.5 pl-[58px] pr-5">
      <div className="grid w-fit grid-cols-[6px_auto_auto] items-center gap-x-2.5 gap-y-1">
        {steps.map((st) => (
          <div key={st.key} className="contents">
            <span
              className={cn("size-1.5 rounded-full", DOT[st.state])}
              style={st.state === "skipped" ? HATCH : undefined}
            />
            <span
              className={cn(
                "text-xs",
                st.state === "pending" ? "text-muted-foreground/60" : "text-foreground",
              )}
            >
              {st.label}
            </span>
            <span className="text-[11px] tabular-nums text-muted-foreground">{st.value}</span>
          </div>
        ))}
      </div>

      {note && (
        <p
          data-selectable={note.bad || undefined}
          className={cn(
            "mt-2 max-w-prose text-xs leading-relaxed",
            note.bad ? "text-destructive" : "text-muted-foreground",
          )}
        >
          {note.text}
        </p>
      )}

      <div className="mt-2.5 flex items-center gap-1.5">
        <span className="mr-auto min-w-0 truncate text-[11px] text-muted-foreground/70">
          {item.relativePath}
        </span>
        {actions.run && onRun && (
          <Button variant="secondary" size="xs" data-tab-skip onClick={onRun}>
            <Play weight="fill" /> {actions.run.label}
          </Button>
        )}
        {actions.skip && onSkip && (
          <Button variant="secondary" size="xs" data-tab-skip onClick={onSkip}>
            <SkipForward /> Skip
          </Button>
        )}
        <Button
          variant="secondary"
          size="xs"
          data-tab-href={filePagePath(item.subjectId, item.relativePath)}
          onClick={() => openItem(item)}
        >
          <SidebarSimple className="scale-x-[-1]" /> Open beside
        </Button>
      </div>
    </div>
  );
}

// ── Rows ──────────────────────────────────────────────────────────────────────

const Row = memo(function Row({
  item,
  embedStage,
  expanded,
  now,
  onToggle,
  onResume,
  onSkip,
}: {
  item: PipelineItem;
  embedStage: boolean;
  expanded: boolean;
  /** The table's ticking clock; a new value re-renders the memoised row so
   *  its relative time and upload estimate don't go stale. */
  now: number;
  onToggle: (path: string) => void;
  onResume?: (item: PipelineItem) => void;
  onSkip?: (item: PipelineItem) => void;
}) {
  // A spreadsheet has no embed stage whatever the app's setting.
  const stages = embedsIn(item, embedStage);
  const s = statusOf(item, stages, now);
  const actions = actionsOf(item, s, stages, !!onResume, !!onSkip);
  const run = onResume && (() => onResume(item));
  const skip = onSkip && (() => onSkip(item));
  const latest = latestStageAt(item);
  const href = filePagePath(item.subjectId, item.relativePath);
  const Icon = fileIconFor(item.filename);

  return (
    <div>
      <div
        role="button"
        tabIndex={0}
        aria-expanded={expanded}
        onClick={() => onToggle(item.relativePath)}
        onKeyDown={(e) => {
          // Keys on the controls inside are theirs.
          if (e.target !== e.currentTarget) return;
          if (e.key === "Enter" || e.key === " ") {
            e.preventDefault();
            onToggle(item.relativePath);
          }
        }}
        className={cn(COLS, "group/row cursor-pointer py-2 transition-colors hover:bg-surface/60")}
      >
        <div className="flex min-w-0 items-center gap-2">
          <CaretRight
            size={9}
            className={cn(
              "shrink-0 text-muted-foreground/50 transition-transform will-change-transform",
              expanded && "rotate-90",
            )}
          />
          <Icon size={13} className="shrink-0 text-muted-foreground/70" />
          <button
            type="button"
            data-tab-href={href}
            onClick={(e) => {
              e.stopPropagation();
              openItem(item);
            }}
            className="min-w-0 truncate rounded-sm text-left text-xs text-foreground decoration-muted-foreground/50 underline-offset-2 outline-none hover:underline focus-visible:ring-2 focus-visible:ring-ring/50"
          >
            {item.filename}
          </button>
          {item.code && (
            <span className="shrink-0 text-[11px] text-muted-foreground/70 @max-lg:hidden">
              {displayCode(item.code)}
            </span>
          )}
        </div>

        <div className="flex min-w-0 items-center gap-3">
          <Track item={item} s={s} embedStage={stages} />
          {isRateLimited(item) ? (
            <RateLimitedCaption item={item} embedStage={stages} />
          ) : (
            <CaptionText text={captionOf(item, s)} tone={s.phase === "failed" ? "bad" : undefined} />
          )}
        </div>

        {/* The time, and on hover or keyboard focus the row's actions in its place. */}
        <div className="relative flex h-6 items-center justify-end">
          <span
            className="text-[11px] tabular-nums text-muted-foreground transition-opacity will-change-[opacity] group-focus-within/row:opacity-0 group-hover/row:opacity-0"
          >
            {latest ? fmtAgo(latest) : "—"}
          </span>
          <div className="absolute inset-y-0 right-0 flex items-center gap-0.5 opacity-0 transition-opacity will-change-[opacity] group-focus-within/row:opacity-100 group-hover/row:opacity-100">
            {actions.run && run && (
              <IconAction hint={actions.run.hint} onClick={run}>
                <Play size={11} weight="fill" />
              </IconAction>
            )}
            {actions.skip && skip && (
              <IconAction hint={SKIP_HINT} onClick={skip}>
                <SkipForward size={12} />
              </IconAction>
            )}
            <IconAction hint="Open beside" onClick={() => openItem(item)}>
              <SidebarSimple size={12} className="scale-x-[-1]" />
            </IconAction>
          </div>
        </div>
      </div>

      {expanded && (
        <Detail item={item} embedStage={stages} actions={actions} onRun={run} onSkip={skip} />
      )}
    </div>
  );
});

/** Sort: running work first, then the queue, then paused, then failures,
 *  then skips; done last. Within a group, the latest stage completion first
 *  — not `updatedAt`, which every progress event bumps, so live rows would
 *  swap places and jump pages. */
const PHASE_RANK: Record<PipelinePhase, number> = {
  active: 0,
  waiting: 1,
  paused: 2,
  failed: 3,
  skipped: 4,
  done: 5,
};

function byActivity(embedStage: boolean) {
  return (a: PipelineItem, b: PipelineItem): number => {
    const ra = PHASE_RANK[statusOf(a, embedStage).phase];
    const rb = PHASE_RANK[statusOf(b, embedStage).phase];
    if (ra !== rb) return ra - rb;
    const ta = latestStageAt(a) || a.startedAt;
    const tb = latestStageAt(b) || b.startedAt;
    if (ta !== tb) return tb - ta;
    return a.relativePath.localeCompare(b.relativePath);
  };
}

/** Files per page; caps how many live rows animate. */
const PAGE_SIZE = 50;

const HEADER = (
  <>
    <span className="text-[11px] font-medium text-muted-foreground">File</span>
    <span className="text-[11px] font-medium text-muted-foreground">Progress</span>
    <span className="justify-self-end text-[11px] font-medium text-muted-foreground">Updated</span>
  </>
);

export const PipelineTable = memo(function PipelineTable({
  items,
  onResume,
  onSkip,
}: {
  items: PipelineItem[];
  onResume?: (item: PipelineItem) => void;
  onSkip?: (item: PipelineItem) => void;
}) {
  const [expanded, setExpanded] = useState<Set<string>>(new Set());
  const embedStage = usePipelineStore((s) => s.embedStage);
  const now = useNow(30_000).getTime();

  const toggle = useCallback((path: string) =>
    setExpanded((prev) => {
      const next = new Set(prev);
      next.has(path) ? next.delete(path) : next.add(path);
      return next;
    }), []);

  const sorted = useMemo(
    () => [...items].sort(byActivity(embedStage)),
    [items, embedStage],
  );
  const { page, pageCount, setPage, pageRows } = usePagedRows(sorted, PAGE_SIZE);

  return (
    <div className="@container h-full">
      <GridTable
        cols={COLS}
        header={HEADER}
        empty={sorted.length === 0 && "Nothing in the pipeline — run a sync to pull new files."}
        pagination={{ page, pageCount, onPage: setPage, total: sorted.length, unit: "file" }}
      >
        <div className="divide-y divide-border-subtle">
          {pageRows.map((it) => (
            <Row
              key={it.relativePath}
              item={it}
              embedStage={embedStage}
              expanded={expanded.has(it.relativePath)}
              now={now}
              onToggle={toggle}
              onResume={onResume}
              onSkip={onSkip}
            />
          ))}
        </div>
      </GridTable>
    </div>
  );
});
