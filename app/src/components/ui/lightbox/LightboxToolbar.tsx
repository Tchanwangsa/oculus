import {
  ArrowsIn,
  MagnifyingGlassMinus,
  MagnifyingGlassPlus,
  X,
} from "@phosphor-icons/react";
import { Button } from "@/components/ui/button";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import type { Limit } from "@/components/ui/lightbox/constants";

/** Floating controls, same grammar as `PDFViewer`'s zoom cluster. The
 *  percentage is a ref so a zoom frame renders no React. */
export function LightboxToolbar({
  readout,
  limit,
  onIn,
  onOut,
  onReset,
  onClose,
}: {
  readout: React.RefObject<HTMLSpanElement | null>;
  limit: Limit;
  onIn: () => void;
  onOut: () => void;
  onReset: () => void;
  onClose: () => void;
}) {
  return (
    <div className="pointer-events-none absolute inset-x-0 bottom-6 flex justify-center">
      <div className="pointer-events-auto flex items-center gap-0.5 rounded-full border border-border bg-card/90 p-1 text-xs text-muted-foreground shadow-md backdrop-blur-sm">
        <Action label="Zoom out" onClick={onOut} disabled={limit === "min"}>
          <MagnifyingGlassMinus size={14} />
        </Action>
        <button
          type="button"
          onClick={onReset}
          className="w-12 cursor-pointer text-center tabular-nums transition-colors hover:text-foreground"
          aria-label="Fit to window"
        >
          {/* Empty on purpose: `paint` owns the text; a JSX child would be
              restored stale on the next `limit` re-render. */}
          <span ref={readout} />
        </button>
        <Action label="Zoom in" onClick={onIn} disabled={limit === "max"}>
          <MagnifyingGlassPlus size={14} />
        </Action>
        <Action label="Fit to window" onClick={onReset}>
          <ArrowsIn size={14} />
        </Action>
        <div className="mx-1 h-4 w-px bg-border" />
        <Action label="Close" onClick={onClose}>
          <X size={14} />
        </Action>
      </div>
    </div>
  );
}

function Action({
  label,
  onClick,
  disabled,
  children,
}: {
  label: string;
  onClick: () => void;
  disabled?: boolean;
  children: React.ReactNode;
}) {
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <Button variant="ghost" size="icon-sm" onClick={onClick} disabled={disabled} aria-label={label}>
          {children}
        </Button>
      </TooltipTrigger>
      <TooltipContent>{label}</TooltipContent>
    </Tooltip>
  );
}
