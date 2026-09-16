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
 * Where a file's parse state is *shown*. The vocabulary itself — which states
 * exist and what each one says — is `app/src/lib/parseState.ts`; these are its
 * two renderings, and they are in one file so the word on a row and the
 * sentence in a document can never drift apart.
 *
 * Only PDF-backed files have a parse, so everything here returns `null` for a
 * Canvas page or an image: a blank column is right for a file that was never
 * going to be parsed, and wrong for one that was.
 */

/**
 * The live state of one file, or `null` when the file has no parse at all.
 *
 * The store is the live word and `files.parse_status` is the standing one, so
 * the row falls back to the column: a file that failed in a *previous* session
 * is only that column (the discriminants are not persisted — deliberately, no
 * migration), and without the fallback it would read as "not parsed yet",
 * which is a different and gentler lie than the one this stage exists to fix.
 */
export function useFileParseState(file: DbFile | null): ParseState | null {
  const path = file?.relative_path ?? "";
  const live = useParseStore((s) => s.statuses[path]);
  const failure = useParseStore((s) => s.failures[path]);
  const latch = useParseStore((s) => s.latch);
  if (!file || !isPdfBacked(file.filename)) return null;
  return parseStateOf(live ?? file.parse_status ?? undefined, failure, latch);
}

/**
 * The parse word on a file row (Downloads).
 *
 * It used to be `"parsed"`, `"failed"`, or the empty string for everything
 * else — so a file that had never been parsed, one queued behind forty others
 * and one that failed for a reason nobody would ever fix all looked exactly
 * like a file that was fine. Every state now has a word, and the reason is one
 * hover away.
 */
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
 * What stands where the PDF ↔ Markdown toggle would be when there is no
 * markdown to toggle to.
 *
 * The toggle simply not rendering was the second invisible failure: the reader
 * got a PDF, no Markdown view they had seen on other files, and nothing at all
 * about why. This says which of the reasons it is — still queued, failed,
 * or parsing is down for the whole library — and, where the fix is a token,
 * offers the page that holds it. `/settings/library` is a real route
 * (`app/src/routes.tsx`); the quota case gets no button because waiting is the
 * only move.
 */
export function MarkdownUnavailable({ file }: { file: DbFile }) {
  const live = useFileParseState(file);
  const [open, setOpen] = useState(false);

  // This is only rendered when the markdown is genuinely not on disk, so a
  // state of `parsed` means the record and the disk disagree — say that
  // rather than rendering nothing, which is the silence being fixed here.
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
