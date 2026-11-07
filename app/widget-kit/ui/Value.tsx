import type { ReactNode } from "react";

/** A labelled readout. Numbers take tabular figures so a changing value
 *  doesn't jog; `digits` fixes the decimals. */
export function Value({
  label,
  value,
  unit,
  digits,
}: {
  label?: ReactNode;
  value: number | string;
  unit?: string;
  digits?: number;
}) {
  const shown = typeof value === "number" ? formatNumber(value, digits) : value;
  return (
    <div className="o-value">
      {label != null && <span className="o-label">{label}</span>}
      <span className="o-value-number">
        {shown}
        {unit && <span className="o-value-unit">{unit}</span>}
      </span>
    </div>
  );
}

export function formatNumber(value: number, digits?: number): string {
  if (!Number.isFinite(value)) return String(value);
  return value.toLocaleString(undefined, digits == null ? { maximumFractionDigits: 4 } : { minimumFractionDigits: digits, maximumFractionDigits: digits });
}
