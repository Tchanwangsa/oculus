import { ColorPicker } from "@/components/ui/color/ColorPicker";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { parseHex } from "@/lib/ui/color";
import { cn } from "@/lib/utils";

/** The checkerboard a see-through colour is drawn over. */
const CHECKER =
  "repeating-conic-gradient(var(--color-border) 0% 25%, transparent 0% 50%) 0 0 / 6px 6px";

/**
 * A colour as a round swatch that opens the picker in a popover, with its hex
 * beside it. `value` is hex text (`#rgb`, `#rrggbb` or `#rrggbbaa`); edits
 * come back as `#rrggbb`, or `#rrggbbaa` when not opaque.
 */
export function ColorField({
  value,
  onChange,
  label,
  className,
}: {
  value: string;
  onChange: (hex: string) => void;
  /** What the colour is, for the swatch's accessible name. */
  label: string;
  className?: string;
}) {
  const ok = parseHex(value) != null;
  return (
    <Popover>
      <PopoverTrigger asChild>
        <button
          type="button"
          aria-label={`${label}: ${value}`}
          className={cn(
            "group flex min-w-0 cursor-pointer items-center gap-2 rounded-full py-0.5 pl-0.5 pr-2 text-left transition-colors hover:bg-accent",
            className,
          )}
        >
          <span
            aria-hidden
            className="size-5 shrink-0 overflow-hidden rounded-full ring-1 ring-border"
            style={{ background: CHECKER }}
          >
            <span className="block size-full" style={{ backgroundColor: ok ? value : "transparent" }} />
          </span>
          <span className={cn("truncate text-[12px] uppercase tabular-nums", ok ? "text-muted-foreground" : "text-destructive")}>
            {value}
          </span>
        </button>
      </PopoverTrigger>
      <PopoverContent align="end" className="w-60 p-3">
        <ColorPicker value={ok ? value : "#000000"} onChange={onChange} />
      </PopoverContent>
    </Popover>
  );
}
