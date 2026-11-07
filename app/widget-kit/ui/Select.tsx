import { useId, type ReactNode } from "react";

export type Option = string | { value: string; label: string };

/** One choice from a list, as a native select in a pill. */
export function Select({
  label,
  value,
  options,
  onChange,
}: {
  label?: ReactNode;
  value: string;
  options: Option[];
  onChange: (value: string) => void;
}) {
  const id = useId();
  return (
    <div className="o-select">
      {label != null && (
        <label className="o-label" htmlFor={id}>
          {label}
        </label>
      )}
      <span className="o-select-box">
        <select id={id} value={value} onChange={(e) => onChange(e.currentTarget.value)}>
          {options.map((o) => {
            const { value: v, label: l } = typeof o === "string" ? { value: o, label: o } : o;
            return (
              <option key={v} value={v}>
                {l}
              </option>
            );
          })}
        </select>
      </span>
    </div>
  );
}
