import { useState } from "react";
import { CircleNotch, Info, WarningCircle } from "@phosphor-icons/react";
import { cn } from "@/lib/utils";
import { Button } from "@/components/ui/button";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { PARSE_TONE_CLASS, parseStateOf, type ParseState } from "@/lib/parseState";
import { isPdfBacked } from "@/lib/fileTypes";
import { navigateActive } from "@/lib/tabRouters";
import { useParseStore } from "@/stores/parseStore";
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

/** The parse word on a file row, with the reason on hover. */
export function ParseStateBadge({
  file,
  className,
}: {
  file: DbFile;
  className?: string;
}) {
  const state = useFileParseState(file);
  const word = (
    <span
      className={cn(
        "text-[10px] uppercase tracking-wide",
        state ? PARSE_TONE_CLASS[state.tone] : undefined,
        className,
      )}
    >
      {state?.label ?? ""}
    </span>
  );
  if (!state || state.kind === "parsed") return word;
  return (
    <Tooltip>
      {/* asChild keeps this a span: the whole row is already a button. */}
      <TooltipTrigger asChild>{word}</TooltipTrigger>
      <TooltipContent side="top" className="max-w-64 text-[11px] leading-snug">
        {state.detail}
      </TooltipContent>
    </Tooltip>
  );
}

const MISSING_ARTIFACT: ParseState = {
  kind: "failed",
  label: "failed",
  title: "No Markdown for this file",
  detail:
    "This file is recorded as parsed, but its markdown could not be read from disk.",
  tone: "bad",
  fixInSettings: false,
};

/**
 * Stands in for the PDF ↔ Markdown toggle when there is no markdown: says
 * why (queued, failed, parsing down) and links to Settings when the fix is
 * there.
 */
export function MarkdownUnavailable({ file }: { file: DbFile }) {
  const live = useFileParseState(file);
  const [open, setOpen] = useState(false);

  // Only rendered when the markdown is missing on disk, so `parsed` here
  // means the record and the disk disagree.
  const state: ParseState =
    !live || live.kind === "parsed" ? MISSING_ARTIFACT : live;

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
        <p className="mt-1 text-xs leading-relaxed text-muted-foreground">
          {state.detail}
        </p>
        {state.fixInSettings && (
          <Button
            variant="secondary"
            size="xs"
            className="mt-3"
            data-tab-href="/settings/library"
            onClick={() => {
              setOpen(false);
              navigateActive("/settings/library");
            }}
          >
            Open Library settings
          </Button>
        )}
      </PopoverContent>
    </Popover>
  );
}
