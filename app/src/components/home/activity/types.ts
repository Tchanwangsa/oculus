import type { ReactNode } from "react";

/** One stacked series as the legend and the hover card draw it. */
export interface Series {
  key: string;
  label: string;
  color: string;
  /** The legend's mark in place of a plain swatch. */
  legendMark?: ReactNode;
  /** The hover card's mark before the label. */
  mark: ReactNode;
}

/** A bar segment, bottom first. */
export interface Segment {
  key: string;
  seconds: number;
  color: string;
}
