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
import { MAX_ZOOM, MIN_ZOOM, type LayoutMode, type Shown } from "./layout";

/** One toolbar press, as a ratio. */
const ZOOM_STEP = 1.1;

interface Props {
  mode: LayoutMode;
  onMode: (mode: LayoutMode) => void;
  numPages: number;
  shown: Shown;
  /** The page box's text: the live range, or the draft while it has focus. */
  pageText: string;
  /** The live range, which sizes the box. */
  pageLabel: string;
  onPageDraft: (draft: string | null) => void;
  onJump: (text: string) => void;
  onPrev: () => void;
  onNext: () => void;
  scale: number;
  onZoom: (factor: number) => void;
  onResetZoom: () => void;
}

/** The one slim control row: layout, page, zoom. */
export function PdfToolbar({
  mode,
  onMode,
  numPages,
  shown,
  pageText,
  pageLabel,
  onPageDraft,
  onJump,
  onPrev,
  onNext,
  scale,
  onZoom,
  onResetZoom,
}: Props) {
  const paged = mode !== "scroll" && numPages > 1;
  return (
  <div className="shrink-0 flex items-center gap-3 px-3 h-9 border-b border-border-subtle bg-surface">
    <ToggleGroup
      type="single"
      value={mode}
      onValueChange={(v) => v && onMode(v as LayoutMode)}
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
            onClick={onPrev}
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
              onPageDraft(String(shown.first));
              e.currentTarget.select();
            }}
            onChange={(e) => onPageDraft(e.target.value.replace(/\D/g, ""))}
            onBlur={() => onPageDraft(null)}
            onKeyDown={(e) => {
              if (e.key === "Enter") {
                e.preventDefault();
                onJump(e.currentTarget.value);
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
            onClick={onNext}
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
        onClick={() => onZoom(1 / ZOOM_STEP)}
        disabled={scale <= MIN_ZOOM}
        aria-label="Zoom out"
      >
        <MagnifyingGlassMinus size={13} />
      </Button>
      <button
        onClick={onResetZoom}
        className="tabular-nums w-11 text-center hover:text-foreground transition-colors"
        aria-label="Fit page"
        title="Fit page"
      >
        {Math.round(scale * 100)}%
      </button>
      <Button
        variant="ghost"
        size="icon-xs"
        onClick={() => onZoom(ZOOM_STEP)}
        disabled={scale >= MAX_ZOOM}
        aria-label="Zoom in"
      >
        <MagnifyingGlassPlus size={13} />
      </Button>
    </div>
  </div>
  );
}

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
