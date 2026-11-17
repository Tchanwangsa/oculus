import { Fragment, useId, type ReactNode } from 'react';
import { arrow, diamond, oval, rectangle, line } from './shapes';
import { eraser, highlighter, pen } from './ink';
import { grab, lasso, select } from './pointer';
import { image, matrix, text } from './insert';
import { xy, xyz } from './graphs';
import type { Part, Shade, ToolArtName } from './types';

const ART: Record<ToolArtName, readonly Part[]> = { select, lasso, grab, pen, highlighter, eraser, text, image, matrix, rectangle, oval, diamond, arrow, line, xy, xyz };
/** The outline of coloured art. Resting art draws in `currentColor` over `--tool-fill`, so the button's hover colour shows through overlaps. */
const INK = '#1a1a1a';
const RESTING_FILL = 'var(--tool-fill, var(--color-card))';

interface Paint { colour: boolean; ink: string; clip: () => string }

/** A tool's art: a rail glyph that is an outline at rest and flat colour with a shaded side when `colour` is set. */
export function ToolArt({ name, colour = false, className }: { name: ToolArtName; colour?: boolean; className?: string }) {
  const prefix = useId().replace(/[^a-zA-Z0-9]/g, '');
  let count = 0;
  const paint: Paint = { colour, ink: colour ? INK : 'currentColor', clip: () => `${prefix}-${count++}` };
  return <svg className={className} viewBox="0 0 512 512" aria-hidden="true" focusable="false" fill="none" strokeWidth={18} strokeLinejoin="round" strokeLinecap="round">
    {ART[name].map((part, index) => <Fragment key={index}>{render(part, paint)}</Fragment>)}
  </svg>;
}

const fillOf = (paint: Paint, colour: string) => paint.colour ? colour : RESTING_FILL;

function shades(d: string, list: readonly Shade[] | undefined, paint: Paint, width?: number): ReactNode {
  if (!paint.colour || !list) return null;
  return list.map(([points, colour]) => {
    const id = paint.clip();
    return <Fragment key={id}>
      <clipPath id={id}><polygon points={points} /></clipPath>
      {width === undefined
        ? <path d={d} fill={colour} stroke="none" clipPath={`url(#${id})`} />
        : <path d={d} stroke={colour} strokeWidth={width} clipPath={`url(#${id})`} />}
    </Fragment>;
  });
}

function render(part: Part, paint: Paint): ReactNode {
  switch (part.kind) {
    case 'shape': return <>
      <path d={part.d} fill={fillOf(paint, part.fill)} stroke="none" />
      {shades(part.d, part.shades, paint)}
      <path d={part.d} stroke={paint.ink} />
    </>;
    case 'rope': return <>
      <path d={part.d} stroke={paint.ink} strokeWidth={part.width} />
      <path d={part.d} stroke={fillOf(paint, part.fill)} strokeWidth={part.width - 36} />
      {shades(part.d, part.shades, paint, part.width - 36)}
    </>;
    case 'frame': return <path d={part.d} stroke={paint.ink} />;
    case 'marquee': return <>
      <path d={`${part.d}Z`} fill={fillOf(paint, part.fill)} stroke="none" />
      <path d={part.d} stroke={paint.ink} strokeWidth={30} strokeLinecap="butt" strokeDasharray="62 44" />
    </>;
    case 'trace': return <>
      <path d={part.silhouette} fill={fillOf(paint, part.fill)} stroke="none" />
      {shades(part.silhouette, part.shades, paint)}
      <path d={part.outline} fill={paint.ink} fillRule="evenodd" stroke="none" />
    </>;
    case 'group': return <g transform={part.transform}>{part.parts.map((inner, index) => <Fragment key={index}>{render(inner, paint)}</Fragment>)}</g>;
  }
}
