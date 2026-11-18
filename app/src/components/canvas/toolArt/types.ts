/** A clip polygon and the colour the part takes inside it. */
export type Shade = readonly [clip: string, colour: string];

/** One piece of a tool's art, drawn in a 512 box with a bold outline. */
export type Part =
  | { kind: 'shape'; d: string; fill: string; shades?: readonly Shade[] }
  /** A thick round-capped stroke with an outline either side. */
  | { kind: 'rope'; d: string; width: number; fill: string; shades?: readonly Shade[] }
  /** An outline drawn over earlier parts, unfilled. */
  | { kind: 'frame'; d: string }
  /** A dashed selection loop over a pale fill. */
  | { kind: 'marquee'; d: string; fill: string }
  /** A traced silhouette plus its ring-shaped ink. */
  | { kind: 'trace'; silhouette: string; outline: string; fill: string; shades?: readonly Shade[] }
  | { kind: 'group'; transform: string; parts: readonly Part[] };

export type ToolArtName =
  | 'select' | 'lasso' | 'grab' | 'pen' | 'highlighter' | 'eraser' | 'text' | 'image' | 'matrix'
  | 'rectangle' | 'oval' | 'diamond' | 'arrow' | 'line' | 'xy' | 'xyz';
