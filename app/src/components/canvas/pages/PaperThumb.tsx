import type { PaperTemplate } from '@/lib/canvas/paper';

export const TEMPLATE_LABELS: Record<PaperTemplate, string> = { blank: 'Blank', grid: 'Grid', ruled: 'Ruled', dotted: 'Dotted' };

/** A small drawing of a page's template: coarse on purpose, since the real pattern is too fine to read at thumbnail size. */
function thumbPath(template: PaperTemplate, width: number, height: number): string {
  let d = '';
  if (template === 'grid') {
    for (let x = 10; x < width; x += 10) d += `M${x} 0V${height}`;
    for (let y = 10; y < height; y += 10) d += `M0 ${y}H${width}`;
  } else if (template === 'ruled') {
    for (let y = 18; y < height; y += 9) d += `M0 ${y}H${width}`;
  } else if (template === 'dotted') {
    for (let x = 10; x < width; x += 10) for (let y = 10; y < height; y += 10) d += `M${x} ${y}h0`;
  }
  return d;
}

/** A sheet of `template` paper, 84px on its long side unless `long` says otherwise. */
export function PaperThumb({ template, orientation, long = 84 }: { template: PaperTemplate; orientation: 'portrait' | 'landscape'; long?: number }) {
  const [width, height] = orientation === 'portrait' ? [60, 84] : [84, 60], scale = long / 84;
  return <svg width={width * scale} height={height * scale} viewBox={`0 0 ${width} ${height}`} aria-hidden="true" className="shrink-0 rounded-xs bg-white shadow-xs ring-1 ring-border">
    <path d={thumbPath(template, width, height)} fill="none" strokeLinecap="round" strokeWidth={template === 'dotted' ? 1.6 : 0.6} className="stroke-muted-foreground/50" />
  </svg>;
}
