import "./motion.css";
import { useContext, type ReactNode } from "react";
import { useIsStreaming } from "@openuidev/react-lang";
import { MoreComing, StreamingStatus } from "../../Streaming";
import { cn } from "@/lib/utils";
import { Frame, Rich } from "../Frame";
import { InCard } from "../layout/inCard";
import { fraction, tickLabel, type Axis } from "./scale";

/** A chart's own card at the top level, or a captioned figure inside one. */
export function ChartFigure({ title, children }: { title: unknown; children: ReactNode }) {
  const inCard = useContext(InCard);
  const streaming = useIsStreaming();
  const name = typeof title === "string" && title ? title : undefined;
  const body = <figure aria-label={name} className="m-0 min-w-0 tabular-nums">{children}</figure>;
  if (!inCard) return <Frame title={name} footer={streaming ? <StreamingStatus /> : undefined}>{body}</Frame>;
  return <div className="my-2">
    {name && <Rich text={name} className="mb-1 block text-[13px] font-medium text-foreground" />}
    {body}
  </div>;
}

/** A plot-sized shimmer for a chart whose data has not arrived yet; settled
 *  with no data it says so instead. */
export function ChartEmpty() {
  const streaming = useIsStreaming();
  if (streaming) return <div className="min-h-40"><MoreComing /></div>;
  return <p className="my-2 text-xs text-muted-foreground">Not enough data to draw.</p>;
}

export type LegendEntry = { label: string; swatch: string; line?: boolean };

/** Shown only for two or more series; a single series is named by its title. */
export function Legend({ entries }: { entries: LegendEntry[] }) {
  if (entries.length < 2) return null;
  return <ul className="mt-2 flex flex-wrap gap-x-4 gap-y-1 text-xs text-muted-foreground">
    {entries.map((e, i) => <li key={i} className="flex items-center gap-1.5">
      <span aria-hidden="true" className={cn("shrink-0 rounded-full", e.line ? "h-0.5 w-3" : "size-2", e.swatch)} />
      <Rich text={e.label} />
    </li>)}
  </ul>;
}

/** The data behind a chart for screen readers: the table view a picture
 *  can't replace. */
export function DataTable({ head, rows }: { head: string[]; rows: (string | number)[][] }) {
  return <table className="sr-only">
    <thead><tr>{head.map((h, i) => <th key={i}>{h}</th>)}</tr></thead>
    <tbody>{rows.map((r, i) => <tr key={i}>{r.map((c, j) => <td key={j}>{c}</td>)}</tr>)}</tbody>
  </table>;
}

export type Tick = { at: number; label: string };

export const axisTicks = (axis: Axis): Tick[] =>
  axis.ticks.filter((t) => t >= axis.lo - 1e-9 && t <= axis.hi + 1e-9).map((t) => ({ at: fraction(axis, t), label: tickLabel(t, axis.step) }));

/**
 * The cartesian frame every x/y chart shares, laid out in CSS: tick labels
 * and gridlines sit at percentages of the plot box and marks are percentage
 * boxes or a stretched SVG with non-scaling strokes, so nothing is measured
 * and page zoom or a narrow pane only reflows it. The header row carries the
 * y-axis title on the left and the hover readout on the right.
 */
export function Plot({ yTicks, xTicks, xLabel, yLabel, readout, onLeave, children, footer }: {
  yTicks: Tick[];
  xTicks: Tick[];
  xLabel?: unknown;
  yLabel?: unknown;
  readout?: ReactNode;
  onLeave?: () => void;
  children: ReactNode;
  footer?: ReactNode;
}) {
  const gutter = Math.max(2, ...yTicks.map((t) => t.label.length)) + 1;
  return <div className="text-[11px] text-muted-foreground">
    <div className="mb-1 flex min-h-4 items-baseline justify-between gap-3">
      <span className="min-w-0 truncate">{typeof yLabel === "string" && yLabel ? <Rich text={yLabel} /> : null}</span>
      <span aria-live="polite" className="shrink-0 tabular-nums text-foreground">{readout}</span>
    </div>
    <div className="flex">
      <div aria-hidden="true" className="relative shrink-0 tabular-nums" style={{ width: `${gutter}ch` }}>
        {yTicks.map((t, i) => <span key={i} className="absolute right-1.5 translate-y-1/2 leading-none" style={{ bottom: `${t.at * 100}%` }}>{t.label}</span>)}
      </div>
      <div className="relative h-40 min-w-0 flex-1 border-b border-border" onPointerLeave={onLeave}>
        {yTicks.map((t, i) => t.at > 0.001 && <div key={i} aria-hidden="true" className="absolute inset-x-0 border-t border-border-subtle" style={{ bottom: `${t.at * 100}%` }} />)}
        {children}
      </div>
    </div>
    <div aria-hidden="true" className="relative h-4 tabular-nums" style={{ marginLeft: `${gutter}ch` }}>
      {xTicks.map((t, i) => <span key={i} className="absolute top-1 whitespace-nowrap leading-none" style={{ left: `${t.at * 100}%`, transform: `translateX(${t.at < 0.04 ? 0 : t.at > 0.96 ? -100 : -50}%)` }}>{t.label}</span>)}
    </div>
    {typeof xLabel === "string" && xLabel && <div className="mt-1 text-center" style={{ marginLeft: `${gutter}ch` }}><Rich text={xLabel} /></div>}
    {footer}
  </div>;
}

/** Invisible full-height hit bands, one per x position, so hovering
 *  anywhere above a point reads it out; no pointer maths, no measuring. */
export function HitBands({ at, onHover }: { at: number[]; onHover: (i: number) => void }) {
  return <>{at.map((f, i) => {
    const left = i === 0 ? 0 : (at[i - 1] + f) / 2;
    const right = i === at.length - 1 ? 1 : (f + at[i + 1]) / 2;
    return <div key={i} aria-hidden="true" className="absolute inset-y-0" style={{ left: `${left * 100}%`, width: `${Math.max(0, right - left) * 100}%` }}
      onPointerEnter={() => onHover(i)} onPointerDown={() => onHover(i)} />;
  })}</>;
}

/** A filled marker with the 2px surface ring that keeps it legible where it
 *  crosses a line, centred on a point of the plot box. */
export function Dot({ fx, fy, fill, className }: { fx: number; fy: number; fill: string; className?: string }) {
  return <span aria-hidden="true" className={cn("pointer-events-none absolute size-2 -translate-x-1/2 translate-y-1/2 rounded-full ring-2 ring-card", fill, className)}
    style={{ left: `${fx * 100}%`, bottom: `${fy * 100}%` }} />;
}

/** A vertical marker line at an x position with a short label (the mean). */
export function Marker({ fx, label }: { fx: number; label: string }) {
  if (!(fx >= 0 && fx <= 1)) return null;
  return <div aria-hidden="true" className="pointer-events-none absolute inset-y-0" style={{ left: `${fx * 100}%` }}>
    <div className="absolute inset-y-0 -ml-px w-0.5 bg-foreground/60" />
    <span className={cn("absolute top-0 whitespace-nowrap bg-card px-1 text-[11px] leading-4 text-foreground", fx > 0.75 ? "right-1" : "left-1")}>{label}</span>
  </div>;
}
