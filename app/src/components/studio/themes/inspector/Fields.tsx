import { useEffect, useState, type ReactNode } from "react";

import { Input } from "@/components/ui/input";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { cn } from "@/lib/utils";

/** `accentStrong` -> `Accent strong`, `h1` -> `H1`: a token name as a label. */
export function tokenLabel(name: string): string {
  const spaced = name.replace(/([a-z])([A-Z])/g, "$1 $2").toLowerCase();
  return spaced.charAt(0).toUpperCase() + spaced.slice(1);
}

/** One group of tokens under a small heading. */
export function Section({ title, children, note }: { title: string; children: ReactNode; note?: ReactNode }) {
  return (
    <section className="space-y-1.5">
      <h3 className="pt-1 text-[11px] font-medium tracking-wide text-muted-foreground">{title}</h3>
      {children}
      {note && <p className="text-[11.5px] leading-snug text-muted-foreground">{note}</p>}
    </section>
  );
}

/** A label on the left, its control on the right. */
export function Row({ label, children, className }: { label: string; children: ReactNode; className?: string }) {
  return (
    <div className={cn("flex min-h-8 items-center gap-2", className)}>
      <span className="min-w-0 flex-1 truncate text-[12.5px] text-foreground" title={label}>
        {label}
      </span>
      {children}
    </div>
  );
}

/** A number token. The text is a draft, so "0." or "-" can be typed on the
 *  way to a number; only a finite number at or above `min` is handed on. */
export function NumberField({
  value,
  onChange,
  label,
  min = 0,
  step = 1,
  unit,
  className,
}: {
  value: number | undefined;
  onChange: (n: number) => void;
  label: string;
  min?: number;
  step?: number;
  /** Shown after the field, e.g. `px` or `s`. */
  unit?: string;
  className?: string;
}) {
  const [draft, setDraft] = useState(value === undefined ? "" : String(value));
  useEffect(() => {
    // An outside change (a revert, another theme) replaces the draft.
    setDraft((d) => (Number(d) === value && d !== "" ? d : value === undefined ? "" : String(value)));
  }, [value]);
  const n = draft.trim() === "" ? NaN : Number(draft);
  const ok = Number.isFinite(n) && n >= min;
  return (
    <span className={cn("flex shrink-0 items-center gap-1", className)}>
      <Input
        aria-label={label}
        type="number"
        inputMode="decimal"
        min={min}
        step={step}
        value={draft}
        aria-invalid={!ok}
        onChange={(e) => {
          setDraft(e.target.value);
          const next = e.target.value.trim() === "" ? NaN : Number(e.target.value);
          if (Number.isFinite(next) && next >= min) onChange(next);
        }}
        onBlur={() => !ok && setDraft(value === undefined ? "" : String(value))}
        className="h-7 w-[72px] px-2 text-right tabular-nums"
      />
      {unit && <span className="w-5 text-[11.5px] text-muted-foreground">{unit}</span>}
    </span>
  );
}

/** One of a closed set of names, as a small select. */
export function NameSelect({
  value,
  options,
  onChange,
  label,
  className,
}: {
  value: string | undefined;
  options: readonly string[];
  onChange: (value: string) => void;
  label: string;
  className?: string;
}) {
  return (
    <Select value={value && options.includes(value) ? value : ""} onValueChange={onChange}>
      <SelectTrigger size="sm" aria-label={label} className={cn("h-7 w-[136px] text-[12.5px]", className)}>
        <SelectValue placeholder={value ?? "Choose"} />
      </SelectTrigger>
      <SelectContent className="max-h-56">
        {options.map((o) => (
          <SelectItem key={o} value={o} className="text-[12.5px]">
            {o}
          </SelectItem>
        ))}
      </SelectContent>
    </Select>
  );
}
