import { Children, type ReactNode } from "react";

/**
 * Every child drawn in one grid cell, only `active` visible. The hidden ones
 * keep their layout (`visibility: hidden`), so the cell is always as tall as
 * the tallest child and switching never changes the frame's height. Hidden
 * children are `inert`: not focusable, not read out.
 */
export function Panels({ children, active }: { children?: ReactNode; active: number }) {
  const panels = Children.toArray(children);
  return (
    <div className="o-panels">
      {panels.map((panel, i) => (
        <div
          key={i}
          className="o-panel"
          style={i === active ? undefined : { visibility: "hidden" }}
          aria-hidden={i === active ? undefined : true}
          // React 19 takes `inert` as a boolean attribute.
          inert={i === active ? undefined : true}
        >
          {panel}
        </div>
      ))}
    </div>
  );
}

/** `i` held inside `0..count-1`. */
export const clampIndex = (i: number, count: number) => (count <= 0 ? 0 : Math.min(count - 1, Math.max(0, Math.trunc(i) || 0)));
