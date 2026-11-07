import { useState, type ReactNode } from "react";
import { Panels, clampIndex } from "./Panels";

/**
 * Views of one thing, one tab per child in `labels` order. Like `Pages`,
 * every panel is laid out at once, so switching never changes the height.
 */
export function Tabs({
  labels,
  children,
  value,
  onChange,
}: {
  labels: string[];
  children?: ReactNode;
  value?: number;
  onChange?: (index: number) => void;
}) {
  const [own, setOwn] = useState(0);
  const current = clampIndex(value ?? own, labels.length);
  const pick = (i: number) => {
    if (value == null) setOwn(i);
    onChange?.(i);
  };
  return (
    <div className="o-tabs">
      <div className="o-tablist" role="tablist">
        {labels.map((label, i) => (
          <button
            key={i}
            type="button"
            role="tab"
            aria-selected={i === current}
            className="o-tab"
            onClick={() => pick(i)}
          >
            {label}
          </button>
        ))}
      </div>
      <Panels active={current}>{children}</Panels>
    </div>
  );
}
