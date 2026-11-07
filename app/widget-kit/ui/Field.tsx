import { useId, type ReactNode } from "react";
import { Tex } from "./Tex";

/**
 * A labelled text input. `formula` draws what was typed as maths under the
 * field (the value is TeX). Never focuses itself: a widget mounting while the
 * student types in the composer must not take the keyboard.
 */
export function Field({
  label,
  value,
  onChange,
  placeholder,
  formula,
  hint,
}: {
  label?: ReactNode;
  value: string;
  onChange: (value: string) => void;
  placeholder?: string;
  formula?: boolean;
  hint?: ReactNode;
}) {
  const id = useId();
  return (
    <div className="o-field">
      {label != null && (
        <label className="o-label" htmlFor={id}>
          {label}
        </label>
      )}
      <input
        id={id}
        type="text"
        value={value}
        placeholder={placeholder}
        spellCheck={false}
        autoComplete="off"
        onChange={(e) => onChange(e.currentTarget.value)}
      />
      {formula && value.trim() && (
        <div className="o-field-preview">
          <Tex>{value}</Tex>
        </div>
      )}
      {hint != null && <p className="o-note">{hint}</p>}
    </div>
  );
}
