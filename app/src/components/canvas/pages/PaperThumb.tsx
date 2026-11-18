import type { PaperTemplate } from '@/lib/canvas/paper';

export const TEMPLATE_LABELS: Record<PaperTemplate, string> = {
  blank: 'Blank', grid: 'Grid', ruled: 'Ruled', dotted: 'Dotted', graph: 'Graph', cornell: 'Cornell', music: 'Music', isometric: 'Isometric',
};

/** A small drawing of a page's template as [fine, bold] path data: coarse on purpose, since the real pattern is too fine to read at thumbnail size. */
function thumbPaths(template: PaperTemplate, width: number, height: number): [string, string] {
  let d = '', bold = '';
  const across = (y: number) => `M0 ${y}H${width}`, down = (x: number) => `M${x} 0V${height}`;
  if (template === 'grid' || template === 'graph') {
    const step = template === 'graph' ? 6 : 10;
    for (let x = step; x < width; x += step) d += down(x);
    for (let y = step; y < height; y += step) d += across(y);
    if (template === 'graph') {
      for (let x = 30; x < width; x += 30) bold += down(x);
      for (let y = 30; y < height; y += 30) bold += across(y);
    }
  } else if (template === 'ruled' || template === 'cornell') {
    for (let y = 18; y < height; y += 9) d += across(y);
    if (template === 'cornell') {
      const summary = 18 + 9 * Math.floor((height * 0.82 - 18) / 9), cue = Math.round(width * 0.28);
      bold = `${across(18)}${across(summary)}M${cue} 18V${summary}`;
    }
  } else if (template === 'dotted') {
    for (let x = 10; x < width; x += 10) for (let y = 10; y < height; y += 10) d += `M${x} ${y}h0`;
  } else if (template === 'music') {
    for (let staff = 14; staff + 12 < height - 6; staff += 22) for (let i = 0; i < 5; i++) d += `M6 ${staff + i * 3}H${width - 6}`;
  } else if (template === 'isometric') {
    const step = 12, run = step * Math.cos(Math.PI / 6), rise = width * Math.tan(Math.PI / 6);
    for (let x = run; x < width; x += run) d += down(Math.round(x * 10) / 10);
    for (let c = -step * Math.ceil(rise / step); c < height; c += step) d += `M0 ${c}L${width} ${Math.round((c + rise) * 10) / 10}`;
    for (let c = 0; c < height + rise; c += step) d += `M0 ${c}L${width} ${Math.round((c - rise) * 10) / 10}`;
  }
  return [d, bold];
}

/** A sheet of `template` paper, 84px on its long side unless `long` says otherwise. */
export function PaperThumb({ template, orientation, long = 84 }: { template: PaperTemplate; orientation: 'portrait' | 'landscape'; long?: number }) {
  const [width, height] = orientation === 'portrait' ? [60, 84] : [84, 60], scale = long / 84;
  const [d, bold] = thumbPaths(template, width, height);
  return <svg width={width * scale} height={height * scale} viewBox={`0 0 ${width} ${height}`} aria-hidden="true" className="shrink-0 overflow-hidden rounded-xs bg-white shadow-xs ring-1 ring-border">
    <path d={d} fill="none" strokeLinecap="round" strokeWidth={template === 'dotted' ? 1.6 : 0.6} className="stroke-muted-foreground/50" />
    {bold ? <path d={bold} fill="none" strokeWidth={1} className="stroke-muted-foreground/80" /> : null}
  </svg>;
}
