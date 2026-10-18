import { useEffect, useRef, useState } from "react";

import { arrowStep, useFractionDrag } from "@/components/ui/color/useFractionDrag";
import { Input } from "@/components/ui/input";
import { hsvToRgb, parseHex, rgbToHsv, toHex, unit, type Hsv } from "@/lib/ui/color";
import { cn } from "@/lib/utils";

/** The checkerboard under a colour with alpha. */
const CHECKER =
  "repeating-conic-gradient(var(--color-border) 0% 25%, transparent 0% 50%) 0 0 / 8px 8px";

/**
 * A colour picker: a saturation/value pad, a hue strip, an alpha strip and
 * the hex text, all editing one hex value (`#rrggbb`, or `#rrggbbaa` when it
 * is not opaque). The hue is held here, so dragging through grey or black
 * does not lose it. `opaque` drops the alpha strip and every edit's alpha.
 */
export function ColorPicker({
  value,
  onChange,
  opaque: solid = false,
  hex = true,
}: {
  value: string;
  onChange: (hex: string) => void;
  opaque?: boolean;
  /** False when the caller shows its own `HexInput`. */
  hex?: boolean;
}) {
  const parsed = parseHex(value) ?? { r: 0, g: 0, b: 0, a: 1 };
  const [hsv, setHsv] = useState<Hsv>(() => rgbToHsv(parsed));
  const alpha = solid ? 1 : parsed.a;
  const sent = useRef(value);

  // An outside change (another edit, a revert) replaces the held colour.
  useEffect(() => {
    if (value === sent.current) return;
    sent.current = value;
    const next = parseHex(value);
    if (next) setHsv(rgbToHsv(next));
  }, [value]);

  const emit = (next: Hsv, a = alpha) => {
    setHsv(next);
    const hex = toHex(hsvToRgb(next, a));
    sent.current = hex;
    onChange(hex);
  };

  const typed = (text: string) => {
    const parsedText = parseHex(text);
    if (!parsedText) return;
    const next = solid ? { ...parsedText, a: 1 } : parsedText;
    setHsv(rgbToHsv(next));
    const hex = toHex(next);
    sent.current = hex;
    onChange(hex);
  };

  const pad = useFractionDrag((x, y) => emit({ ...hsv, s: x, v: 1 - y }));
  const hue = useFractionDrag((x) => emit({ ...hsv, h: x * 360 }));
  const fade = useFractionDrag((x) => emit(hsv, Math.round(x * 100) / 100));
  const opaque = toHex(hsvToRgb(hsv, 1));

  return (
    <div className="space-y-2.5">
      <div
        role="slider"
        tabIndex={0}
        aria-label="Saturation and brightness"
        aria-valuetext={`saturation ${Math.round(hsv.s * 100)}%, brightness ${Math.round(hsv.v * 100)}%`}
        {...pad}
        onKeyDown={(e) => {
          const step = arrowStep(e, 0.01);
          if (!step) return;
          e.preventDefault();
          emit({ ...hsv, s: unit(hsv.s + step[0]), v: unit(hsv.v - step[1]) });
        }}
        className="relative h-36 w-full cursor-crosshair touch-none rounded-lg outline-none focus-visible:ring-[3px] focus-visible:ring-ring/50"
        style={{
          background: `linear-gradient(to top, #000, transparent), linear-gradient(to right, #fff, hsl(${hsv.h} 100% 50%))`,
        }}
      >
        <Thumb x={hsv.s} y={1 - hsv.v} color={opaque} />
      </div>
      <Strip
        label="Hue"
        valueText={`${Math.round(hsv.h)} degrees`}
        drag={hue}
        at={hsv.h / 360}
        background="linear-gradient(to right, #f00, #ff0, #0f0, #0ff, #00f, #f0f, #f00)"
        thumb={`hsl(${hsv.h} 100% 50%)`}
        onStep={(dx) => emit({ ...hsv, h: (hsv.h + dx * 360 + 360) % 360 })}
        step={1 / 360}
      />
      {!solid && (
        <Strip
          label="Opacity"
          valueText={`${Math.round(alpha * 100)}%`}
          drag={fade}
          at={alpha}
          background={`linear-gradient(to right, transparent, ${opaque}), ${CHECKER}`}
          thumb={toHex(hsvToRgb(hsv, alpha))}
          onStep={(dx) => emit(hsv, Math.round(unit(alpha + dx) * 100) / 100)}
          step={0.01}
        />
      )}
      {hex && <HexInput value={value} onCommit={typed} />}
    </div>
  );
}

function Thumb({ x, y, color }: { x: number; y: number; color: string }) {
  return (
    <span
      aria-hidden
      className="pointer-events-none absolute size-3.5 -translate-x-1/2 -translate-y-1/2 rounded-full border-2 border-white shadow-[0_0_0_1px_rgba(0,0,0,0.3)]"
      style={{ left: `${x * 100}%`, top: `${y * 100}%`, backgroundColor: color }}
    />
  );
}

function Strip({
  label,
  valueText,
  drag,
  at,
  background,
  thumb,
  onStep,
  step,
}: {
  label: string;
  valueText: string;
  drag: ReturnType<typeof useFractionDrag>;
  at: number;
  background: string;
  thumb: string;
  onStep: (dx: number) => void;
  step: number;
}) {
  return (
    <div
      role="slider"
      tabIndex={0}
      aria-label={label}
      aria-valuetext={valueText}
      {...drag}
      onKeyDown={(e) => {
        const s = arrowStep(e, step);
        if (!s) return;
        e.preventDefault();
        onStep(s[0] || -s[1]);
      }}
      className="relative h-3 w-full cursor-pointer touch-none rounded-full ring-1 ring-border outline-none focus-visible:ring-[3px] focus-visible:ring-ring/50"
      style={{ background }}
    >
      <Thumb x={at} y={0.5} color={thumb} />
    </div>
  );
}

/** Six or eight digits are a whole colour; three may still be the start of
 *  six, so they are taken only when the field is left. */
const whole = (text: string) => /^#?([0-9a-f]{6}|[0-9a-f]{8})$/i.test(text.trim());

/** A value that is not hex (`transparent`) shows as an empty field. */
const shown = (value: string) => (parseHex(value) ? value : "");

/** The hex text: a draft while typing, taken once it is a whole colour (or
 *  on leaving the field, `#rgb` too), put back when left holding something
 *  that is not a colour. */
export function HexInput({ value, onCommit }: { value: string; onCommit: (hex: string) => void }) {
  const [draft, setDraft] = useState(() => shown(value));
  useEffect(() => {
    // Keep what is being typed when it already names this colour.
    setDraft((d) => {
      const typed = parseHex(d);
      return typed && toHex(typed) === value ? d : shown(value);
    });
  }, [value]);
  const valid = parseHex(draft) != null;
  const bad = !valid && draft.trim() !== "";
  return (
    <Input
      aria-label="Hex colour"
      value={draft}
      spellCheck={false}
      onChange={(e) => {
        setDraft(e.target.value);
        if (whole(e.target.value)) onCommit(e.target.value);
      }}
      onBlur={() => (valid ? onCommit(draft) : setDraft(shown(value)))}
      onKeyDown={(e) => {
        if (e.key === "Enter" && valid) onCommit(draft);
      }}
      aria-invalid={bad}
      placeholder="#rrggbb"
      className={cn("h-7 tabular-nums", bad && "text-destructive")}
    />
  );
}
