import type { ReactNode } from "react";

/** An on/off switch with its label. */
export function Toggle({
  label,
  checked,
  onChange,
  disabled,
}: {
  label?: ReactNode;
  checked: boolean;
  onChange: (checked: boolean) => void;
  disabled?: boolean;
}) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      disabled={disabled}
      className="o-toggle"
      onClick={() => onChange(!checked)}
    >
      <span className="o-toggle-track">
        <span className="o-toggle-knob" />
      </span>
      {label != null && <span className="o-toggle-label">{label}</span>}
    </button>
  );
}
