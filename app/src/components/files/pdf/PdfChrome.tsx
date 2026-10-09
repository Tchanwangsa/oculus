import {
  BookOpen,
  CaretLeft,
  CaretRight,
  File as FileIcon,
  MagnifyingGlassMinus,
  MagnifyingGlassPlus,
  Rows,
} from "@phosphor-icons/react";
import { Button } from "@/components/ui/button";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { MAX_ZOOM, MIN_ZOOM, ZOOM_STEP } from "@/components/files/pdf/constants";
import type { LayoutMode, Shown } from "@/components/files/pdf/types";

function ModeItem({
  value, label, children,
}: {
  value: LayoutMode;
  label: string;
  children: React.ReactNode;
}) {
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <ToggleGroupItem
          value={value}
          aria-label={label}
          className="h-6 px-2 data-[state=on]:bg-primary data-[state=on]:text-primary-foreground"
        >
          {children}
        </ToggleGroupItem>
      </TooltipTrigger>
      <TooltipContent>{label}</TooltipContent>
    </Tooltip>
  );
}

interface PdfChromeProps {
  mode: LayoutMode;
  setMode: (mode: LayoutMode) => void;
  numPages: number;
  shown: Shown;
  /** The page box's text while it has focus; null shows the live range. */
  pageDraft: string | null;
  setPageDraft: (draft: string | null) => void;
  prev: () => void;
  next: () => void;
  jumpTo: (text: string) => void;
  scale: number;
  zoomBy: (factor: number) => void;
  resetZoom: () => void;
}

/** One slim control row: layout · page · zoom. */
export function PdfChrome({
  mode,
  setMode,
  numPages,
  shown,
  pageDraft,
  setPageDraft,
  prev,
  next,
  jumpTo,
  scale,
  zoomBy,
  resetZoom,
}: PdfChromeProps) {
  // Idle, the box names every page on screen; focused, it holds just the first
  // page to edit. The total is fixed text beside it.
  const pageLabel =
    shown.first === shown.last ? String(shown.first) : `${shown.first}–${shown.last}`;
  const pageText = pageDraft ?? pageLabel;
  const paged = mode !== "scroll" && numPages > 1;

  return (
    <div className="shrink-0 flex items-center gap-3 px-3 h-9 border-b border-border-subtle bg-surface">
      <ToggleGroup
        type="single"
        value={mode}
        onValueChange={(v) => v && setMode(v as LayoutMode)}
        variant="outline"
        size="sm"
        className="shrink-0"
      >
        <ModeItem value="scroll" label="Continuous scroll">
          <Rows size={12} />
        </ModeItem>
        <ModeItem value="single" label="Single page">
          <FileIcon size={12} />
        </ModeItem>
        <ModeItem value="spread" label="Two-page spread">
          <BookOpen size={12} />
        </ModeItem>
      </ToggleGroup>

      {numPages > 0 && (
        <div className="flex items-center gap-1 text-xs text-muted-foreground">
          {paged && (
            <Button
              variant="ghost"
              size="icon-xs"
              disabled={shown.first <= 1}
              onClick={prev}
              aria-label="Previous page"
            >
              <CaretLeft size={13} />
            </Button>
          )}
          <label className="flex h-6 cursor-text items-center gap-1 rounded-md border border-input px-2 text-xs tabular-nums text-muted-foreground shadow-none transition-[color,box-shadow] focus-within:border-ring focus-within:text-foreground focus-within:ring-[3px] focus-within:ring-ring/25 dark:bg-input/30">
            <input
              value={pageText}
              aria-label="Page"
              inputMode="numeric"
              spellCheck={false}
              autoComplete="off"
              // Focus by hand so the click doesn't drop a caret into the
              // selection `onFocus` makes.
              onMouseDown={(e) => {
                if (document.activeElement === e.currentTarget) return;
                e.preventDefault();
                e.currentTarget.focus();
              }}
              onFocus={(e) => {
                setPageDraft(String(shown.first));
                e.currentTarget.select();
              }}
              onChange={(e) => setPageDraft(e.target.value.replace(/\D/g, ""))}
              onBlur={() => setPageDraft(null)}
              onKeyDown={(e) => {
                if (e.key === "Enter") {
                  e.preventDefault();
                  jumpTo(e.currentTarget.value);
                  e.currentTarget.blur();
                } else if (e.key === "Escape") {
                  e.stopPropagation();
                  e.currentTarget.blur();
                }
              }}
              // Sized to the idle label so focusing doesn't resize it.
              style={{
                width: `${Math.max(pageText.length, pageLabel.length)}ch`,
              }}
              className="bg-transparent p-0 text-center outline-none"
            />
            <span aria-hidden>/ {numPages}</span>
          </label>
          {paged && (
            <Button
              variant="ghost"
              size="icon-xs"
              disabled={shown.last >= numPages}
              onClick={next}
              aria-label="Next page"
            >
              <CaretRight size={13} />
            </Button>
          )}
        </div>
      )}

      <div className="ml-auto flex items-center gap-1 text-xs text-muted-foreground">
        <Button
          variant="ghost"
          size="icon-xs"
          onClick={() => zoomBy(1 / ZOOM_STEP)}
          disabled={scale <= MIN_ZOOM}
          aria-label="Zoom out"
        >
          <MagnifyingGlassMinus size={13} />
        </Button>
        <button
          onClick={resetZoom}
          className="tabular-nums w-11 text-center hover:text-foreground transition-colors"
          aria-label="Fit page"
          title="Fit page"
        >
          {Math.round(scale * 100)}%
        </button>
        <Button
          variant="ghost"
          size="icon-xs"
          onClick={() => zoomBy(ZOOM_STEP)}
          disabled={scale >= MAX_ZOOM}
          aria-label="Zoom in"
        >
          <MagnifyingGlassPlus size={13} />
        </Button>
      </div>
    </div>
  );
}
