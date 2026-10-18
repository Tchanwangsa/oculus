import { useRef, useState, type ReactNode } from "react";

import { ColorPicker } from "@/components/ui/color/ColorPicker";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { parseHex } from "@/lib/ui/color";
import { useScrollFade } from "@/hooks/ui/useScrollFade";
import { cn } from "@/lib/utils";

/** The checkerboard a see-through colour is drawn over. */
const CHECKER =
  "repeating-conic-gradient(var(--color-border) 0% 25%, transparent 0% 50%) 0 0 / 6px 6px";

/** A colour as a round dot, over the checkerboard so alpha shows. */
export function ColorSwatch({ value, className }: { value: string; className?: string }) {
  return (
    <span
      aria-hidden
      className={cn("size-5 shrink-0 overflow-hidden rounded-full ring-1 ring-border", className)}
      style={{ background: CHECKER }}
    >
      <span className="block size-full" style={{ backgroundColor: parseHex(value) != null ? value : "transparent" }} />
    </span>
  );
}

/**
 * A button that opens the colour picker in a popover. `children` is the
 * button's face (`ColorField` draws a swatch and the hex, a tile may draw
 * more); edits come back as `#rrggbb`, or `#rrggbbaa` when not opaque.
 */
export function ColorPopover({
  value,
  onChange,
  label,
  align = "end",
  side = "bottom",
  opaque,
  className,
  children,
}: {
  value: string;
  onChange: (hex: string) => void;
  /** What the colour is, for the button's accessible name. */
  label: string;
  align?: "start" | "center" | "end";
  side?: "top" | "right" | "bottom" | "left";
  /** No alpha: edits always come back as `#rrggbb`. */
  opaque?: boolean;
  className?: string;
  children: ReactNode;
}) {
  const ok = parseHex(value) != null;
  const [open, setOpen] = useState(false);
  const contentRef = useRef<HTMLDivElement>(null);
  useScrollFade(contentRef, "y", open);
  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger asChild>
        <button type="button" aria-label={`${label}: ${value}`} className={cn("cursor-pointer text-left", className)}>
          {children}
        </button>
      </PopoverTrigger>
      <PopoverContent
        ref={contentRef}
        align={align}
        side={side}
        collisionPadding={12}
        className="w-60 max-w-[var(--radix-popover-content-available-width)] max-h-[var(--radix-popover-content-available-height)] overflow-y-auto p-3"
      >
        <ColorPicker value={ok ? value : "#000000"} onChange={onChange} opaque={opaque} />
      </PopoverContent>
    </Popover>
  );
}

/**
 * A colour as a round swatch that opens the picker in a popover, with its hex
 * beside it. `value` is hex text (`#rgb`, `#rrggbb` or `#rrggbbaa`); edits
 * come back as `#rrggbb`, or `#rrggbbaa` when not opaque.
 */
export function ColorField({
  value,
  onChange,
  label,
  align,
  side,
  opaque,
  className,
}: {
  value: string;
  onChange: (hex: string) => void;
  label: string;
  align?: "start" | "center" | "end";
  side?: "top" | "right" | "bottom" | "left";
  opaque?: boolean;
  className?: string;
}) {
  const ok = parseHex(value) != null;
  return (
    <ColorPopover
      value={value}
      onChange={onChange}
      label={label}
      align={align}
      side={side}
      opaque={opaque}
      className={cn(
        "group flex min-w-0 items-center gap-2 rounded-full py-0.5 pl-0.5 pr-2 transition-colors hover:bg-accent",
        className,
      )}
    >
      <ColorSwatch value={value} />
      <span className={cn("truncate text-[12px] uppercase tabular-nums", ok ? "text-muted-foreground" : "text-destructive")}>
        {value}
      </span>
    </ColorPopover>
  );
}
