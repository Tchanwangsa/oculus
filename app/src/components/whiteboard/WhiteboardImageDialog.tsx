import { useCallback, useEffect, useRef, useState } from 'react';
import { CaretLeftIcon, FilePdfIcon, ImageIcon, SpinnerGapIcon, XIcon } from '@phosphor-icons/react';
import { Button } from '@/components/ui/button';
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from '@/components/ui/dialog';
import { useScrollFade } from '@/hooks/ui/useScrollFade';
import { assertWhiteboardImportSize, WHITEBOARD_IMAGE_ACCEPT, MAX_WHITEBOARD_IMPORT_IMAGES, readWhiteboardImages, type WhiteboardImportedImage, type WhiteboardImportPdf } from '@/lib/whiteboard/importMedia';
import { useWhiteboardPlatform, type WhiteboardLibraryPdf } from '@/lib/whiteboard/platform';
import type { DrawingKind } from '@/lib/whiteboard/drawingKinds';
import { kindNoun } from '@/lib/whiteboard/kindWords';
import { WhiteboardPdfPages } from './WhiteboardPdfPages';
import { WhiteboardLibraryPdfs } from './WhiteboardLibraryPdfs';

type Busy = 'images' | 'pdf' | 'import' | null;
const message = (error: unknown) => error instanceof Error ? error.message : String(error);

/** A notebook's "Insert PDF pages": only library PDFs, and the chosen pages come back with their sizes in points. */
export interface PdfPagesTarget {
  /** The page number (from 1) the new pages follow, for the dialog's wording. */
  after: number;
  onInsert: (sizes: { width: number; height: number }[], images: WhiteboardImportedImage[]) => void | Promise<void>;
}

/** Add image: local images or library PDF pages as images. With `pdfPages`, it picks PDF pages to become notebook pages.
 *  `kind` names the drawing in the text. */
export function WhiteboardImageDialog({ open, onOpenChange, onImport, pdfPages, kind }: {
  open: boolean;
  kind: DrawingKind;
  onOpenChange: (open: boolean) => void;
  onImport?: (images: WhiteboardImportedImage[]) => void | Promise<void>;
  pdfPages?: PdfPagesTarget;
}) {
  const platform = useWhiteboardPlatform();
  const noun = kindNoun(kind);
  const pagesOnly = !!pdfPages;
  const [browsingPdfs, setBrowsingPdfs] = useState(pagesOnly);
  const [images, setImages] = useState<WhiteboardImportedImage[]>([]);
  const [pdf, setPdf] = useState<WhiteboardImportPdf | null>(null);
  const [selected, setSelected] = useState<Set<number>>(new Set());
  const [busy, setBusy] = useState<Busy>(null);
  const [progress, setProgress] = useState('');
  const [error, setError] = useState<string | null>(null);
  const input = useRef<HTMLInputElement>(null);
  const scroller = useRef<HTMLDivElement>(null);
  const source = useRef<WhiteboardImportPdf | null>(null);
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
    setBusy(null); setProgress(''); setError(null); setBrowsingPdfs(pagesOnly);
  }, [release, pagesOnly]);
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
      const imported = await readWhiteboardImages(files, abort.signal);
      if (token === generation.current) setImages(imported);
    } catch (problem) {
      if (token === generation.current) setError(message(problem));
    } finally {
      if (token === generation.current) { setBusy(null); controller.current = null; }
    }
  };
  const choosePdf = async (file: WhiteboardLibraryPdf) => {
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
    if (busy || (!images.length && !selected.size) || selected.size > MAX_WHITEBOARD_IMPORT_IMAGES) return;
    const token = generation.current;
    setBusy('import'); setError(null);
    try {
      const imported = [...images];
      const pages = pdf ? [...selected].sort((a, b) => a - b) : [];
      if (pdf) {
        for (let index = 0; index < pages.length; index++) {
          if (token !== generation.current) return;
          setProgress(`Preparing page ${index + 1} of ${pages.length}…`);
          imported.push(await pdf.renderPage(pages[index]));
          assertWhiteboardImportSize(imported);
        }
      }
      if (token !== generation.current) return;
      assertWhiteboardImportSize(imported);
      if (pdfPages && pdf) {
        setProgress('Adding pages…');
        await pdfPages.onInsert(pages.map(page => pdf.pages[page - 1]), imported);
      } else {
        setProgress(`Adding to ${noun}…`);
        await onImport?.(imported);
      }
      if (token === generation.current) changeOpen(false);
    } catch (problem) {
      if (token === generation.current) setError(message(problem));
    } finally {
      if (token === generation.current) { setBusy(null); setProgress(''); }
    }
  };
  const count = pdf ? selected.size : images.length;
  const selectionError = count > MAX_WHITEBOARD_IMPORT_IMAGES ? `Choose up to ${MAX_WHITEBOARD_IMPORT_IMAGES} pages at a time. Clear the selection and choose fewer pages.` : null;

  return <Dialog open={open} onOpenChange={changeOpen}>
    <DialogContent className="flex max-h-[calc(100dvh-2rem)] flex-col sm:max-w-2xl" onKeyDown={event => event.stopPropagation()} onPointerDown={event => event.stopPropagation()}>
      <DialogHeader>
        <DialogTitle>{pdfPages ? 'Insert PDF pages' : 'Add images'}</DialogTitle>
        <DialogDescription>{pdfPages ? `Each page you select becomes a new page after page ${pdfPages.after}, at the PDF page's size, to write on. The PDF itself is not changed.` : 'Choose your own images or select pages from a PDF in your library.'}</DialogDescription>
      </DialogHeader>
      <input ref={input} className="hidden" type="file" accept={WHITEBOARD_IMAGE_ACCEPT} multiple aria-label="Choose image files"
        onChange={event => { const files = Array.from(event.currentTarget.files ?? []); event.currentTarget.value = ''; void chooseImages(files); }} />
      {pagesOnly ? null : <div className="grid grid-cols-1 gap-2 min-[420px]:grid-cols-2">
        <Button variant="outline" className="h-auto justify-start rounded-xl px-4 py-3 text-left" disabled={busy !== null} onClick={() => input.current?.click()}>
          <ImageIcon size={22} className="text-muted-foreground" />
          <span className="flex min-w-0 flex-col gap-0.5"><span>Choose images</span><span className="text-xs font-normal text-muted-foreground">PNG, JPG, WebP or GIF</span></span>
        </Button>
        <Button variant="outline" className="h-auto justify-start rounded-xl px-4 py-3 text-left" disabled={busy !== null} onClick={() => { reset(); setBrowsingPdfs(true); }}>
          <FilePdfIcon size={22} className="text-reference-pdf" />
          <span className="flex min-w-0 flex-col gap-0.5"><span>Choose PDF</span><span className="text-xs font-normal text-muted-foreground">Search your library</span></span>
        </Button>
      </div>}
      {pagesOnly && pdf ? <Button variant="ghost" size="xs" className="self-start" disabled={busy !== null} onClick={reset}><CaretLeftIcon />Choose another PDF</Button> : null}
      {browsingPdfs && <WhiteboardLibraryPdfs disabled={busy !== null} onChoose={file => void choosePdf(file)} />}
      {busy === 'images' || busy === 'pdf' ? <div className="flex items-center gap-2 py-5 text-sm text-muted-foreground" role="status"><SpinnerGapIcon className="animate-spin" />{busy === 'images' ? 'Reading images…' : 'Opening PDF…'}</div> : null}
      {pdf || images.length ? <div ref={scroller} className="min-h-0 overflow-y-auto py-1">
        {pdf ? <WhiteboardPdfPages key={pdf.name} pdf={pdf} selected={selected} onSelected={setSelected} disabled={busy !== null} /> : <div className="grid grid-cols-2 gap-3 sm:grid-cols-3" aria-label="Selected images">
          {images.map((image, index) => <div key={`${index}-${image.name}`} className="relative min-w-0 rounded-lg border border-border-subtle p-2">
            <div className="flex h-28 items-center justify-center rounded bg-surface-raised"><img src={image.dataUrl} alt={image.name} className="h-full w-full object-contain" draggable={false} /></div>
            <p className="mt-2 truncate pr-1 text-xs" title={image.name}>{image.name}</p>
            <Button variant="secondary" size="icon-xs" className="absolute top-3 right-3" aria-label={`Remove ${image.name}`} disabled={busy !== null} onClick={() => setImages(previous => previous.filter((_, at) => at !== index))}><XIcon /></Button>
          </div>)}
        </div>}
      </div> : null}
      {(selectionError || error) && <p className="text-[13px] text-destructive" data-selectable role="alert">{selectionError ?? error}</p>}
      <div className="flex flex-wrap items-center justify-between gap-3 border-t border-border-subtle pt-3">
        <p className="text-xs text-muted-foreground" role="status" aria-live="polite">{busy === 'import' ? progress : count ? `${count} ${pdf ? (count === 1 ? 'page' : 'pages') : (count === 1 ? 'image' : 'images')} selected` : pdf ? 'Click pages to select them.' : pagesOnly ? `Pages are copied into your ${noun}.` : `Images are embedded in your ${noun}.`}</p>
        <DialogFooter className="flex-row">
          <Button variant="outline" onClick={() => changeOpen(false)}>Cancel</Button>
          <Button disabled={busy !== null || count === 0 || selectionError !== null} onClick={() => void importImages()}>
            {busy === 'import' && <SpinnerGapIcon className="animate-spin" aria-hidden />}
            {pdfPages ? 'Insert pages' : pdf ? 'Import selected' : `Add to ${noun}`}
          </Button>
        </DialogFooter>
      </div>
    </DialogContent>
  </Dialog>;
}
