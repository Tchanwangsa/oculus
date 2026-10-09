import { useEffect, useState } from "react";
import {
  CheckCircle,
  CircleDashed,
  CircleNotch,
  Clock,
  Info,
  PauseCircle,
  Prohibit,
  SkipForwardCircle,
  WarningCircle,
  type Icon as PhosphorIcon,
} from "@phosphor-icons/react";
import { cn } from "@/lib/utils";
import { Button } from "@/components/ui/button";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { PARSE_TONE_CLASS, parseStateOf, type ParseState } from "@/lib/parseState";
import { isPdfBacked } from "@/lib/fileTypes";
import { navigateActive } from "@/lib/tabRouters";
import { openFileParseDetails } from "@/lib/openFile";
import { parseFile, parseSkipped } from "@/lib/courseFiles";
import { useParseStore } from "@/stores/parseStore";
import { usePipelineStore } from "@/stores/pipelineStore";
import type { DbFile } from "@/lib/db";

/**
 * The two renderings of a file's parse state (vocabulary in
 * `app/src/lib/parseState.ts`). Only PDF-backed files have one; everything
 * here is `null` for anything else.
 */

/**
 * Live state from the store, falling back to `files.parse_status` — a failure
 * from a previous session exists only in that column.
 */
function useFileParseState(file: DbFile | null): ParseState | null {
  const path = file?.relative_path ?? "";
  const live = useParseStore((s) => s.statuses[path]);
  const failure = useParseStore((s) => s.failures[path]);
  const latch = useParseStore((s) => s.latch);
  if (!file || !isPdfBacked(file.filename)) return null;
  return parseStateOf(live ?? file.parse_status ?? undefined, failure, latch);
}

const STATE_ICON: Record<ParseState["kind"], PhosphorIcon> = {
  parsed: CheckCircle,
  running: CircleNotch,
  queued: Clock,
  skipped: SkipForwardCircle,
  unparsed: CircleDashed,
  failed: WarningCircle,
  permanent: Prohibit,
  blocked: PauseCircle,
};

/** What clicking a state does: re-kick this file's parse, or open Settings
 *  when the fix is there. A latched or permanent failure has nothing to retry. */
type ParseAction = { hint: string; run: () => void };

/** A file row's click: a failure opens the file, whose header shows the
 *  backend's whole message and the retry; everything else as on the file. */
function rowActionOf(
  state: ParseState,
  file: DbFile,
  latched: boolean,
): ParseAction | null {
  if (!state.fixInSettings && (state.kind === "failed" || state.kind === "permanent")) {
    return {
      hint: state.kind === "failed" ? "Click to see why and retry" : "Click to see why",
      run: () => openFileParseDetails(file),
    };
  }
  return parseActionOf(state, file, latched);
}

function parseActionOf(
  state: ParseState,
  file: DbFile,
  latched: boolean,
): ParseAction | null {
  if (state.fixInSettings) {
    return {
      hint: "Click to open Parsing settings",
      run: () => navigateActive("/settings/parsing"),
    };
  }
  if (!latched && (state.kind === "failed" || state.kind === "unparsed" || state.kind === "skipped")) {
    return {
      hint: state.kind === "failed" ? "Click to retry" : "Click to parse now",
      run: () => reparse(file, state.kind === "skipped"),
    };
  }
  return null;
}

/** Shows the file as queued at once; the backend's `parse-status` events
 *  take over from there. A skipped file has its mark lifted first. */
function reparse(file: DbFile, skipped: boolean) {
  const { update } = useParseStore.getState();
  const ev = { relative_path: file.relative_path, subject_id: file.subject_id };
  update({ ...ev, status: "queued" });
  // The pipeline row too, or its skip would swallow the parse's events.
  const pipeline = usePipelineStore.getState();
  if (skipped && pipeline.items[file.relative_path]) {
    pipeline.touch(file.relative_path, file.subject_id, { parse: "queued", skippedAt: undefined });
  }
  // `parse_file` ignores the subject code.
  (skipped ? parseSkipped : parseFile)(file.subject_id, "", file.relative_path).catch((e) =>
    update({ ...ev, status: "error", error: String(e) }),
  );
}

/** The parse icon on a file row: the state and a short reason on hover, and
 *  a click that acts on it. Sits outside the row's own button. */
export function ParseStateBadge({
  file,
  className,
}: {
  file: DbFile;
  className?: string;
}) {
  const state = useFileParseState(file);
  const latched = useParseStore((s) => s.latch != null);
  if (!state) return null;
  const Icon = STATE_ICON[state.kind];
  const action = rowActionOf(state, file, latched);
  const icon = (
    <Icon
      size={13}
      weight={state.kind === "parsed" ? "fill" : "regular"}
      className={state.kind === "running" ? "animate-spin" : undefined}
    />
  );
  const tone = PARSE_TONE_CLASS[state.tone];
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        {action ? (
          <button
            type="button"
            aria-label={`${state.title}. ${action.hint}`}
            onClick={action.run}
            className={cn("inline-flex rounded-sm hover:opacity-70", tone, className)}
          >
            {icon}
          </button>
        ) : (
          <span aria-label={state.title} className={cn("inline-flex", tone, className)}>
            {icon}
          </span>
        )}
      </TooltipTrigger>
      <TooltipContent side="top" className="max-w-64 text-[11px] leading-snug">
        <span className="font-medium">{state.title}</span>
        {state.summary && <span className="block opacity-70">{state.summary}</span>}
        {action && <span className="block opacity-70">{action.hint}</span>}
      </TooltipContent>
    </Tooltip>
  );
}

const MISSING_ARTIFACT: ParseState = {
  kind: "failed",
  label: "failed",
  title: "Markdown missing",
  detail: "Recorded as parsed, but the markdown isn't on disk.",
  summary: "",
  tone: "bad",
  fixInSettings: false,
};

/**
 * Stands in for the PDF ↔ Markdown toggle when there is no markdown: says
 * why (queued, failed, parsing down) and offers the state's action.
 */
export function MarkdownUnavailable({
  file,
  openSeq,
}: {
  file: DbFile;
  /** Set when the page was opened for its parse details (a row's icon). */
  openSeq?: number;
}) {
  const live = useFileParseState(file);
  const latched = useParseStore((s) => s.latch != null);
  const [open, setOpen] = useState(openSeq != null);
  useEffect(() => {
    if (openSeq != null) setOpen(true);
  }, [openSeq]);

  // Only rendered when the markdown is missing on disk, so `parsed` here
  // means the record and the disk disagree.
  const state: ParseState =
    !live || live.kind === "parsed" ? MISSING_ARTIFACT : live;

  const action = parseActionOf(state, file, latched);
  const moving = state.kind === "queued" || state.kind === "running";
  const Icon = moving ? CircleNotch : state.tone === "quiet" ? Info : WarningCircle;

  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger asChild>
        <Button
          variant="ghost"
          size="xs"
          /* Opt out of the row's ⌘-click (`lib/newTabClicks.ts`). */
          data-tab-skip
          className={cn(
            "shrink-0 text-[11px] font-normal",
            moving ? "text-brand" : "text-muted-foreground",
          )}
        >
          <Icon size={11} className={moving ? "animate-spin" : undefined} />
          {moving ? "Parsing…" : "No Markdown"}
        </Button>
      </PopoverTrigger>
      <PopoverContent align="end" className="w-80 p-3.5">
        <p className="text-[13px] font-medium text-foreground">{state.title}</p>
        {state.detail ? (
          <p data-selectable className="mt-1 text-xs leading-relaxed text-muted-foreground">
            {state.detail}
          </p>
        ) : (
          state.kind === "skipped" && (
            <p className="mt-1 text-xs leading-relaxed text-muted-foreground">{state.summary}</p>
          )
        )}
        {action && (
          <Button
            variant="secondary"
            size="xs"
            className="mt-3"
            data-tab-href={state.fixInSettings ? "/settings/parsing" : undefined}
            onClick={() => {
              setOpen(false);
              action.run();
            }}
          >
            {state.fixInSettings ? "Open Parsing settings" : state.kind === "failed" ? "Retry" : "Parse now"}
          </Button>
        )}
      </PopoverContent>
    </Popover>
  );
}
