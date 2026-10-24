import { createCanvasDocument, serializeCanvasDocument, type CanvasDocument, type CanvasElement } from './model';
import { createPaper, pageRects, type Paper, type PaperTemplate } from './paper';
import { createCanvas } from './files';
import { openCanvasLibraryPdf } from './importPdf';
import { MAX_CANVAS_IMPORT_BYTES, MAX_CANVAS_IMPORT_IMAGES, type CanvasImportedImage } from './importMedia';
import { secureId } from './ids';
import type { DbFile } from '@/lib/db';

/** The desk a notebook's pages lie on: the document's `background`. A neutral grey, so white paper reads as paper. */
export const NOTEBOOK_DESK = '#e4e5ea';

export type NotebookOrientation = 'portrait' | 'landscape';

export function blankNotebook(template: PaperTemplate, orientation: NotebookOrientation = 'portrait'): CanvasDocument {
  return { ...createCanvasDocument(), background: NOTEBOOK_DESK, paper: createPaper(template, orientation) };
}

/** A notebook with one blank page per PDF page, each page's picture laid on it as a locked image the size of the page. */
export function notebookFromPages(sizes: readonly { width: number; height: number }[], images: readonly CanvasImportedImage[]): CanvasDocument {
  if (!sizes.length || sizes.length !== images.length) throw new Error('A notebook needs one picture for each page.');
  const paper: Paper = { pages: sizes.map((size, i) => ({ id: `p${i + 1}`, width: size.width, height: size.height, template: 'blank' })) };
  const elements: CanvasElement[] = pageRects(paper).map((rect, i) => ({
    id: secureId(), type: 'image', x: rect.x, y: rect.y, width: rect.width, height: rect.height,
    dataUrl: images[i].dataUrl, sourceName: images[i].name, naturalWidth: images[i].width, naturalHeight: images[i].height,
    stroke: 'transparent', fill: 'transparent', strokeWidth: 0, style: 'clean', seed: 0, locked: true,
  }));
  return { ...createCanvasDocument(), background: NOTEBOOK_DESK, paper, elements };
}

type Subject = { id: number; code: string };

export const createBlankNotebook = (subject: Subject, title: string, template: PaperTemplate, orientation: NotebookOrientation): Promise<DbFile> =>
  createCanvas(subject, title, serializeCanvasDocument(blankNotebook(template, orientation)));

/**
 * A new notebook on a copy of a library PDF's pages. The PDF is only read: its pages are rendered into
 * the notebook file, so the original is never touched and the notebook outlives it.
 */
export async function createNotebookFromPdf(
  subject: Subject, relativePath: string, title: string, onProgress?: (done: number, total: number) => void,
): Promise<DbFile> {
  const pdf = await openCanvasLibraryPdf(relativePath);
  try {
    const total = pdf.pages.length;
    if (total > MAX_CANVAS_IMPORT_IMAGES) throw new Error(`This PDF has ${total} pages; a notebook can start from up to ${MAX_CANVAS_IMPORT_IMAGES}.`);
    const images: CanvasImportedImage[] = [];
    let bytes = 0;
    for (let page = 1; page <= total; page++) {
      onProgress?.(page - 1, total);
      const image = await pdf.renderPage(page);
      bytes += image.dataUrl.length;
      if (bytes > MAX_CANVAS_IMPORT_BYTES) throw new Error('This PDF is too large to start a notebook from: its pages come to more than 64 MB.');
      images.push(image);
    }
    onProgress?.(total, total);
    return await createCanvas(subject, title, serializeCanvasDocument(notebookFromPages(pdf.pages, images)));
  } finally {
    void pdf.close().catch(() => {});
  }
}
