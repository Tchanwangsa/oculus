import { useState } from "react";
import { CaretDownIcon } from "@phosphor-icons/react";

import { ColorPicker, HexInput } from "@/components/ui/color/ColorPicker";
import { parseHex, toHex } from "@/lib/ui/color";
import { cn } from "@/lib/utils";

/** A palette entry: the colour it picks and, for a hue, its shades lightest first (`colorShades`). */
export interface PaletteColor {
  name: string;
  color: string;
  shades?: string[];
}

/** A swatch's paint; `transparent` draws as a red slash, "no colour". */
export const swatchBackground = (color: string) =>
  color === "transparent"
    ? "linear-gradient(135deg, transparent 46%, var(--color-destructive) 47%, var(--color-destructive) 53%, transparent 54%)"
    : color;
const same = (a: string, b: string) => a.toLowerCase() === b.toLowerCase();

/** A colour to pick, as a rounded square; the picked one carries an outline that leaves a gap. */
export function PaletteSwatch({
  color,
  name,
  label,
  selected,
  onPick,
  className,
}: {
  color: string;
  name: string;
  /** What is being coloured, for the accessible name. */
  label: string;
  selected: boolean;
  onPick: (color: string) => void;
  className?: string;
}) {
  return (
    <button
      type="button"
      title={name}
      aria-label={`${label}: ${name}`}
      aria-pressed={selected}
      onClick={() => onPick(color)}
      className={cn(
        "size-6 shrink-0 cursor-pointer rounded-md border border-black/10 outline-none focus-visible:ring-2 focus-visible:ring-brand focus-visible:ring-offset-2 focus-visible:ring-offset-popover",
        selected && "outline-2 outline-offset-2 outline-brand outline-solid",
        className,
      )}
      style={{ background: swatchBackground(color) }}
    />
  );
}

/**
 * A palette of colours, the shades of the hue the value belongs to, and the hex
 * text; Custom opens the full picker for anything else. Picks come back as the
 * palette's own value (`transparent` included) or `#rrggbb` when `opaque`.
 */
export function ColorPalette({
  value,
  onChange,
  colors,
  label,
  columns = 5,
  opaque,
}: {
  value: string;
  onChange: (color: string) => void;
  colors: PaletteColor[];
  label: string;
  columns?: number;
  opaque?: boolean;
}) {
  const [custom, setCustom] = useState(false);
  const hue = colors.find((c) => same(c.color, value) || c.shades?.some((s) => same(s, value)));
  const typed = (text: string) => {
    const parsed = parseHex(text);
    if (parsed) onChange(toHex(opaque ? { ...parsed, a: 1 } : parsed));
  };
  return (
    <div className="space-y-3 text-xs">
      <div className="space-y-2">
        <div className="text-muted-foreground">Colors</div>
        <div role="group" aria-label={`${label} palette`} className="grid gap-1.5" style={{ gridTemplateColumns: `repeat(${columns}, 1.75rem)` }}>
          {colors.map((c) => (
            <PaletteSwatch key={c.name} {...c} label={label} selected={same(c.color, value)} onPick={onChange} className="size-7" />
          ))}
        </div>
      </div>
      <div className="space-y-2">
        <div className="text-muted-foreground">Shades</div>
        {hue?.shades ? (
          <div role="group" aria-label={`${hue.name} shades`} className="grid gap-1.5" style={{ gridTemplateColumns: `repeat(${columns}, 1.75rem)` }}>
            {hue.shades.map((shade, i) => (
              <PaletteSwatch key={i} color={shade} name={`${hue.name} ${i + 1}`} label={label} selected={same(shade, value)} onPick={onChange} className="size-7" />
            ))}
          </div>
        ) : (
          <div className="flex h-7 items-center text-muted-foreground/70">None for this colour</div>
        )}
      </div>
      <div className="space-y-2">
        <div className="flex items-center justify-between text-muted-foreground">
          <span>Hex code</span>
          <button
            type="button"
            aria-expanded={custom}
            onClick={() => setCustom((open) => !open)}
            className="-mr-1 flex cursor-pointer items-center gap-0.5 rounded-full px-1.5 py-0.5 outline-none hover:bg-accent hover:text-foreground focus-visible:ring-2 focus-visible:ring-brand"
          >
            Custom
            <CaretDownIcon size={11} className={cn("transition-transform", custom && "rotate-180")} />
          </button>
        </div>
        <HexInput value={value} onCommit={typed} />
        {custom ? <ColorPicker value={parseHex(value) ? value : "#000000"} onChange={onChange} opaque={opaque} hex={false} /> : null}
      </div>
    </div>
  );
}
