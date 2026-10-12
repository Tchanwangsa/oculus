import { useEffect, useState, type ReactNode } from "react";

import { Input } from "@/components/ui/input";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { cn } from "@/lib/utils";

// The inspectors' fields, 28px tall and quiet: a grey fill that shows its
// border only on hover or focus, squarer than the app's pills so a column of
// them reads as a form, not a stack of buttons.

/** Added to an `Input` or `Textarea`. `dark:bg-secondary` beats the base
 *  field's dark fill. */
export const FIELD = "h-7 rounded-md border-transparent bg-secondary px-2 text-[12.5px] shadow-none hover:border-input dark:bg-secondary";

/** Added to a `SelectTrigger`; the `data-` variants beat the trigger's own
 *  size and hover rules. */
export const SELECT_FIELD =
  "w-full gap-1.5 rounded-md border-transparent bg-secondary px-2 text-[12.5px] shadow-none hover:border-input hover:bg-secondary data-[size=sm]:h-7 dark:bg-secondary dark:hover:bg-secondary";

/** A small quiet heading inside a section, for a run of related fields. */
export function SubLabel({ children, className }: { children: ReactNode; className?: string }) {
  return <div className={cn("pb-1.5 pt-1 text-[11px] font-medium text-muted-foreground", className)}>{children}</div>;
}

/** A label above its field, for the cells of a grid. */
export function LabeledField({ label, children, className }: { label: string; children: ReactNode; className?: string }) {
  return (
    <div className={cn("min-w-0 space-y-1", className)}>
      <div className="truncate text-[11px] text-muted-foreground" title={label}>
        {label}
      </div>
      {children}
    </div>
  );
}

/** A label on the left, its field on the right, on one 28px line. */
export function InlineField({ label, children, className }: { label: string; children: ReactNode; className?: string }) {
  return (
    <div className={cn("flex min-h-7 items-center gap-2", className)}>
      <span className="w-[38%] min-w-0 shrink-0 truncate text-[12.5px] text-foreground" title={label}>
        {label}
      </span>
      <div className="min-w-0 flex-1">{children}</div>
    </div>
  );
}

const NO_SPINNER = "[appearance:textfield] [&::-webkit-inner-spin-button]:appearance-none [&::-webkit-outer-spin-button]:appearance-none";

/**
 * A number field. The text is a draft, so "0." or "-" can be typed on the
 * way to a number; only a finite number at or above `min` is handed on. With
 * `onClear`, an emptied field hands that back instead (a prop that goes
 * unset); without it, an empty field is invalid and reverts on blur.
 */
export function NumberInput({
  value,
  onChange,
  onClear,
  label,
  min,
  step = 1,
  unit,
  prefix,
  placeholder,
  className,
}: {
  value: number | undefined;
  onChange: (n: number) => void;
  onClear?: () => void;
  /** The field's accessible name. */
  label: string;
  min?: number;
  step?: number;
  /** Quiet text at the field's end, e.g. `px` or `s`. */
  unit?: string;
  /** Quiet text at the field's start, e.g. `x1`. */
  prefix?: string;
  placeholder?: string;
  className?: string;
}) {
  const show = (v: number | undefined) => (v === undefined ? "" : String(v));
  const [draft, setDraft] = useState(show(value));
  useEffect(() => {
    // An outside change (a revert, a reset, another example) replaces the draft.
    setDraft((d) => (Number(d) === value && d !== "" ? d : show(value)));
  }, [value]);
  const read = (text: string) => (text.trim() === "" ? NaN : Number(text));
  const valid = (n: number) => Number.isFinite(n) && (min === undefined || n >= min);
  const empty = draft.trim() === "";
  const ok = valid(read(draft)) || (empty && onClear != null);
  return (
    <span className={cn("relative block min-w-0", className)}>
      {prefix && (
        <span aria-hidden className="pointer-events-none absolute left-2 top-1/2 -translate-y-1/2 text-[10.5px] text-muted-foreground">
          {prefix}
        </span>
      )}
      <Input
        aria-label={label}
        title={label}
        type="number"
        inputMode="decimal"
        min={min}
        step={step}
        value={draft}
        placeholder={placeholder}
        aria-invalid={!ok}
        onChange={(e) => {
          setDraft(e.target.value);
          const n = read(e.target.value);
          if (valid(n)) onChange(n);
          else if (e.target.value.trim() === "" && onClear) onClear();
        }}
        onBlur={() => !ok && setDraft(show(value))}
        className={cn(FIELD, "tabular-nums", NO_SPINNER, prefix && "pl-6", unit && "pr-6")}
      />
      {unit && (
        <span aria-hidden className="pointer-events-none absolute right-2 top-1/2 -translate-y-1/2 text-[10.5px] text-muted-foreground">
          {unit}
        </span>
      )}
    </span>
  );
}

/** One of a closed set of names, as a compact select. */
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
      <SelectTrigger size="sm" aria-label={label} className={cn(SELECT_FIELD, className)}>
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
