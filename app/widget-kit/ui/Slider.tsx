import { useId, type CSSProperties, type ReactNode } from "react";
import { formatNumber } from "./Value";

/** A labelled range with its value on the right. The filled part of the
 *  track is `--fill`, set from the value. */
export function Slider({
  label,
  value,
  onChange,
  min = 0,
  max = 1,
  step = 0.01,
  unit,
  digits,
}: {
  label?: ReactNode;
  value: number;
  onChange: (value: number) => void;
  min?: number;
  max?: number;
  step?: number;
  unit?: string;
  digits?: number;
}) {
  const id = useId();
  const fill = max > min ? ((Math.min(max, Math.max(min, value)) - min) / (max - min)) * 100 : 0;
  return (
    <div className="o-slider">
      <div className="o-control-head">
        {label != null && (
          <label className="o-label" htmlFor={id}>
            {label}
          </label>
        )}
        <span className="o-control-value">
          {formatNumber(value, digits)}
          {unit && <span className="o-value-unit">{unit}</span>}
        </span>
      </div>
      <input
        id={id}
        type="range"
        min={min}
        max={max}
        step={step}
        value={value}
        style={{ "--fill": `${fill}%` } as CSSProperties}
        onChange={(e) => onChange(Number(e.currentTarget.value))}
      />
    </div>
  );
}
