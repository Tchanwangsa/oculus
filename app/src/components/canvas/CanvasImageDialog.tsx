import { useCallback, useEffect, useRef, useState } from 'react';
import { FilePdfIcon, ImageIcon, SpinnerGapIcon, XIcon } from '@phosphor-icons/react';
import { Button } from '@/components/ui/button';
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from '@/components/ui/dialog';
import { useScrollFade } from '@/hooks/ui/useScrollFade';
import { assertCanvasImportSize, CANVAS_IMAGE_ACCEPT, MAX_CANVAS_IMPORT_IMAGES, readCanvasImages, type CanvasImportedImage, type CanvasImportPdf } from '@/lib/canvas/importMedia';
import { useCanvasPlatform, type CanvasLibraryPdf } from '@/lib/canvas/platform';
import { CanvasPdfPages } from './CanvasPdfPages';
import { CanvasLibraryPdfs } from './CanvasLibraryPdfs';

type Busy = 'images' | 'pdf' | 'import' | null;
const message = (error: unknown) => error instanceof Error ? error.message : String(error);

export function CanvasImageDialog({ open, onOpenChange, onImport }: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onImport: (images: CanvasImportedImage[]) => void | Promise<void>;
}) {
  const platform = useCanvasPlatform();
  const [browsingPdfs, setBrowsingPdfs] = useState(false);
  const [images, setImages] = useState<CanvasImportedImage[]>([]);
  const [pdf, setPdf] = useState<CanvasImportPdf | null>(null);
  const [selected, setSelected] = useState<Set<number>>(new Set());
  const [busy, setBusy] = useState<Busy>(null);
  const [progress, setProgress] = useState('');
  const [error, setError] = useState<string | null>(null);
  const input = useRef<HTMLInputElement>(null);
  const scroller = useRef<HTMLDivElement>(null);
  const source = useRef<CanvasImportPdf | null>(null);
  const generation = useRef(0);
  const controller = useRef<AbortController | null>(null);
  useScrollFade(scroller, 'y', open && (pdf ?? images));

  const release = useCallback(() => {
    generation.current++;
    controller.current?.abort();
    controller.current = null;
    const previous = source.current;
    source.current = null;
    if (previous) void previous.close().catch(() => {});
  }, []);
  const reset = useCallback(() => {
    release();
    setImages([]); setPdf(null); setSelected(new Set());
    setBusy(null); setProgress(''); setError(null); setBrowsingPdfs(false);
  }, [release]);
  const changeOpen = (next: boolean) => {
    if (!next) reset();
    onOpenChange(next);
  };
  useEffect(() => {
    if (!open) reset();
    return release;
  }, [open, release, reset]);

  const chooseImages = async (files: File[]) => {
    if (!files.length) return;
    reset();
    const token = generation.current;
    const abort = new AbortController();
    controller.current = abort;
    setBusy('images');
    try {
      const imported = await readCanvasImages(files, abort.signal);
      if (token === generation.current) setImages(imported);
    } catch (problem) {
      if (token === generation.current) setError(message(problem));
    } finally {
      if (token === generation.current) { setBusy(null); controller.current = null; }
    }
  };
  const choosePdf = async (file: CanvasLibraryPdf) => {
    reset();
    const token = generation.current;
    setBrowsingPdfs(true);
    setBusy('pdf');
    try {
      const chosen = await platform.openPdf(file.relativePath);
      if (token !== generation.current) {
        if (chosen) void chosen.close().catch(() => {});
        return;
      }
      source.current = chosen;
      setPdf(chosen);
      setBrowsingPdfs(false);
    } catch (problem) {
      if (token === generation.current) setError(message(problem));
    } finally {
      if (token === generation.current) setBusy(null);
    }
  };
  const importImages = async () => {
    if (busy || (!images.length && !selected.size) || selected.size > MAX_CANVAS_IMPORT_IMAGES) return;
    const token = generation.current;
    setBusy('import'); setError(null);
    try {
      const imported = [...images];
      if (pdf) {
        const pages = [...selected].sort((a, b) => a - b);
        for (let index = 0; index < pages.length; index++) {
          if (token !== generation.current) return;
          setProgress(`Preparing page ${index + 1} of ${pages.length}…`);
          imported.push(await pdf.renderPage(pages[index]));
          assertCanvasImportSize(imported);
        }
      }
      if (token !== generation.current) return;
      assertCanvasImportSize(imported);
      setProgress('Adding to canvas…');
      await onImport(imported);
      if (token === generation.current) changeOpen(false);
    } catch (problem) {
      if (token === generation.current) setError(message(problem));
    } finally {
      if (token === generation.current) { setBusy(null); setProgress(''); }
    }
  };
  const count = pdf ? selected.size : images.length;
  const selectionError = count > MAX_CANVAS_IMPORT_IMAGES ? `Choose up to ${MAX_CANVAS_IMPORT_IMAGES} pages at a time. Clear the selection and choose fewer pages.` : null;

  return <Dialog open={open} onOpenChange={changeOpen}>
    <DialogContent className="flex max-h-[calc(100dvh-2rem)] flex-col sm:max-w-2xl" onKeyDown={event => event.stopPropagation()} onPointerDown={event => event.stopPropagation()}>
      <DialogHeader>
        <DialogTitle>Add images</DialogTitle>
        <DialogDescription>Choose your own images or select pages from a PDF in your library.</DialogDescription>
      </DialogHeader>
      <input ref={input} className="hidden" type="file" accept={CANVAS_IMAGE_ACCEPT} multiple aria-label="Choose image files"
        onChange={event => { const files = Array.from(event.currentTarget.files ?? []); event.currentTarget.value = ''; void chooseImages(files); }} />
      <div className="grid grid-cols-1 gap-2 min-[420px]:grid-cols-2">
        <Button variant="outline" className="h-auto justify-start rounded-xl px-4 py-3 text-left" disabled={busy !== null} onClick={() => input.current?.click()}>
          <ImageIcon size={22} className="text-muted-foreground" />
          <span className="flex min-w-0 flex-col gap-0.5"><span>Choose images</span><span className="text-xs font-normal text-muted-foreground">PNG, JPG, WebP or GIF</span></span>
        </Button>
        <Button variant="outline" className="h-auto justify-start rounded-xl px-4 py-3 text-left" disabled={busy !== null} onClick={() => { reset(); setBrowsingPdfs(true); }}>
          <FilePdfIcon size={22} className="text-reference-pdf" />
          <span className="flex min-w-0 flex-col gap-0.5"><span>Choose PDF</span><span className="text-xs font-normal text-muted-foreground">Search your library</span></span>
        </Button>
      </div>
      {browsingPdfs && <CanvasLibraryPdfs disabled={busy !== null} onChoose={file => void choosePdf(file)} />}
      {busy === 'images' || busy === 'pdf' ? <div className="flex items-center gap-2 py-5 text-sm text-muted-foreground" role="status"><SpinnerGapIcon className="animate-spin" />{busy === 'images' ? 'Reading images…' : 'Opening PDF…'}</div> : null}
      {pdf || images.length ? <div ref={scroller} className="min-h-0 overflow-y-auto py-1">
        {pdf ? <CanvasPdfPages key={pdf.name} pdf={pdf} selected={selected} onSelected={setSelected} disabled={busy !== null} /> : <div className="grid grid-cols-2 gap-3 sm:grid-cols-3" aria-label="Selected images">
          {images.map((image, index) => <div key={`${index}-${image.name}`} className="relative min-w-0 rounded-lg border border-border-subtle p-2">
            <div className="flex h-28 items-center justify-center rounded bg-surface-raised"><img src={image.dataUrl} alt={image.name} className="h-full w-full object-contain" draggable={false} /></div>
            <p className="mt-2 truncate pr-1 text-xs" title={image.name}>{image.name}</p>
            <Button variant="secondary" size="icon-xs" className="absolute top-3 right-3" aria-label={`Remove ${image.name}`} disabled={busy !== null} onClick={() => setImages(previous => previous.filter((_, at) => at !== index))}><XIcon /></Button>
          </div>)}
        </div>}
      </div> : null}
      {(selectionError || error) && <p className="text-[13px] text-destructive" data-selectable role="alert">{selectionError ?? error}</p>}
      <div className="flex flex-wrap items-center justify-between gap-3 border-t border-border-subtle pt-3">
        <p className="text-xs text-muted-foreground" role="status" aria-live="polite">{busy === 'import' ? progress : count ? `${count} ${pdf ? (count === 1 ? 'page' : 'pages') : (count === 1 ? 'image' : 'images')} selected` : pdf ? 'Click pages to select them.' : 'Images are embedded in your canvas.'}</p>
        <DialogFooter className="flex-row">
          <Button variant="outline" onClick={() => changeOpen(false)}>Cancel</Button>
          <Button disabled={busy !== null || count === 0 || selectionError !== null} onClick={() => void importImages()}>
            {busy === 'import' && <SpinnerGapIcon className="animate-spin" aria-hidden />}
            {pdf ? 'Import selected' : 'Add to canvas'}
          </Button>
        </DialogFooter>
      </div>
    </DialogContent>
  </Dialog>;
}
