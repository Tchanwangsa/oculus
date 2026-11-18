import { useEffect, useState } from 'react';
import { CaretLeftIcon, CaretRightIcon, CheckIcon, SpinnerGapIcon } from '@phosphor-icons/react';
import { Button } from '@/components/ui/button';
import type { CanvasImportPdf } from '@/lib/canvas/importMedia';
import { cn } from '@/lib/utils';

const PAGE_WINDOW = 12;

type Preview = { dataUrl: string } | { error: true };

/** A bounded page window avoids rasterizing a whole PDF just to choose its pages. */
export function CanvasPdfPages({ pdf, selected, onSelected, disabled }: {
  pdf: CanvasImportPdf;
  selected: Set<number>;
  onSelected: (pages: Set<number>) => void;
  disabled: boolean;
}) {
  const [windowIndex, setWindowIndex] = useState(0);
  const [previews, setPreviews] = useState<Map<number, Preview>>(new Map());
  const first = windowIndex * PAGE_WINDOW + 1;
  const last = Math.min(first + PAGE_WINDOW - 1, pdf.pages.length);
  const pages = Array.from({ length: last - first + 1 }, (_, index) => first + index);

  useEffect(() => {
    let current = true;
    setPreviews(new Map());
    void (async () => {
      for (let page = first; page <= last; page++) {
        if (!current) return;
        let preview: Preview;
        try { preview = { dataUrl: (await pdf.renderPage(page, true)).dataUrl }; }
        catch { preview = { error: true }; }
        if (!current) return;
        setPreviews(previous => new Map(previous).set(page, preview));
      }
    })();
    return () => { current = false; };
  }, [pdf, first, last]);

  const toggle = (page: number) => {
    const next = new Set(selected);
    if (next.has(page)) next.delete(page); else next.add(page);
    onSelected(next);
  };

  return <div className="space-y-3">
    <div className="flex flex-wrap items-center justify-between gap-2">
      <div className="min-w-0 flex-1">
        <p className="truncate text-sm font-medium" title={pdf.name}>{pdf.name}</p>
        <p className="mt-0.5 text-xs text-muted-foreground">{pdf.pages.length} pages · {selected.size} selected</p>
      </div>
      <div className="flex gap-1">
        <Button size="xs" variant="ghost" disabled={disabled || selected.size === pdf.pages.length} onClick={() => onSelected(new Set(pdf.pages.map((_, index) => index + 1)))}>Select all</Button>
        <Button size="xs" variant="ghost" disabled={disabled || selected.size === 0} onClick={() => onSelected(new Set())}>Clear</Button>
      </div>
    </div>
    <div className="grid grid-cols-3 gap-2 sm:grid-cols-4" aria-label="PDF pages">
      {pages.map(page => {
        const preview = previews.get(page), checked = selected.has(page);
        return <button type="button" key={page} disabled={disabled} aria-label={`Page ${page}`} aria-pressed={checked}
          onClick={() => toggle(page)}
          className={cn('relative flex min-w-0 cursor-pointer flex-col gap-2 rounded-lg border p-2 text-xs outline-none transition-colors focus-visible:ring-2 focus-visible:ring-ring disabled:pointer-events-none disabled:opacity-60', checked ? 'border-brand bg-brand/8' : 'border-border-subtle hover:bg-accent')}>
          <span className="relative flex h-28 w-full items-center justify-center overflow-hidden rounded bg-surface-raised">
            {preview && 'dataUrl' in preview ? <img src={preview.dataUrl} alt="" className="h-full w-full object-contain" draggable={false} />
              : preview ? <span className="px-2 text-center text-[11px] text-muted-foreground">Preview unavailable</span>
                : <SpinnerGapIcon className="animate-spin text-muted-foreground" size={20} aria-hidden />}
            {checked && <span className="absolute top-1 right-1 flex size-5 items-center justify-center rounded-full bg-primary text-primary-foreground"><CheckIcon size={12} weight="bold" aria-hidden /></span>}
          </span>
          <span className="self-start tabular-nums">Page {page}</span>
        </button>;
      })}
    </div>
    {pdf.pages.length > PAGE_WINDOW && <div className="flex items-center justify-between gap-2">
      <p className="text-xs text-muted-foreground tabular-nums">Pages {first}–{last} of {pdf.pages.length}</p>
      <div className="flex gap-1">
        <Button variant="outline" size="icon-sm" aria-label="Previous PDF pages" disabled={disabled || windowIndex === 0} onClick={() => setWindowIndex(index => index - 1)}><CaretLeftIcon /></Button>
        <Button variant="outline" size="icon-sm" aria-label="Next PDF pages" disabled={disabled || last === pdf.pages.length} onClick={() => setWindowIndex(index => index + 1)}><CaretRightIcon /></Button>
      </div>
    </div>}
  </div>;
}
