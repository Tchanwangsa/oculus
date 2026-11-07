import type { CSSProperties, ReactNode } from "react";
import { cx } from "./cx";

type Align = "start" | "center" | "end" | "stretch" | "baseline";
type Justify = "start" | "center" | "end" | "between";
const ALIGN: Record<Align, string> = { start: "flex-start", center: "center", end: "flex-end", stretch: "stretch", baseline: "baseline" };
const JUSTIFY: Record<Justify, string> = { start: "flex-start", center: "center", end: "flex-end", between: "space-between" };

/** The widget's root: the card's padding and a column of its parts. */
export function Widget({ children, gap = 12 }: { children?: ReactNode; gap?: number }) {
  return (
    <div className="o-widget" style={{ gap }}>
      {children}
    </div>
  );
}

/** Children side by side, wrapping onto the next line when the frame is narrow. */
export function Row({
  children,
  gap = 8,
  align = "center",
  justify = "start",
  wrap = true,
}: {
  children?: ReactNode;
  gap?: number;
  align?: Align;
  justify?: Justify;
  wrap?: boolean;
}) {
  const style: CSSProperties = {
    gap,
    alignItems: ALIGN[align],
    justifyContent: JUSTIFY[justify],
    flexWrap: wrap ? "wrap" : "nowrap",
  };
  return (
    <div className="o-row" style={style}>
      {children}
    </div>
  );
}

/** Children stacked. `flex` lets it share a `Row` with its siblings. */
export function Column({
  children,
  gap = 8,
  align = "stretch",
  flex,
}: {
  children?: ReactNode;
  gap?: number;
  align?: Align;
  flex?: number;
}) {
  return (
    <div className="o-column" style={{ gap, alignItems: ALIGN[align], flex }}>
      {children}
    </div>
  );
}

/** A short line of explanation under or beside a control. */
export function Note({ children, tone = "muted" }: { children?: ReactNode; tone?: "muted" | "success" | "warning" | "danger" }) {
  return <p className={cx("o-note", tone !== "muted" && `o-note-${tone}`)}>{children}</p>;
}
