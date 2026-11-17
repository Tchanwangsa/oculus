import { createCanvasDocument, serializeCanvasDocument, type CanvasDocument, type CanvasElement } from './model';
import { createPaper, pageRects, type Paper, type PaperTemplate } from './paper';
import { createNotebook } from './files';
import { readPdfPages } from './pdfCopy';
import type { CanvasImportedImage } from './importMedia';
import { pageBackdrop } from './pages';
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
  const elements: CanvasElement[] = pageRects(paper).map((rect, i) => pageBackdrop(rect, images[i]));
  return { ...createCanvasDocument(), background: NOTEBOOK_DESK, paper, elements };
}

type Subject = { id: number; code: string };

export const createBlankNotebook = (subject: Subject, title: string, template: PaperTemplate, orientation: NotebookOrientation, targetDir?: string): Promise<DbFile> =>
  createNotebook(subject, title, serializeCanvasDocument(blankNotebook(template, orientation)), targetDir);

/**
 * A new notebook on a copy of a library PDF's pages. The PDF is only read: its pages are rendered into
 * the notebook file, so the original is never touched and the notebook outlives it.
 */
export async function createNotebookFromPdf(
  subject: Subject, relativePath: string, title: string, onProgress?: (done: number, total: number) => void, targetDir?: string,
): Promise<DbFile> {
  const { sizes, images } = await readPdfPages(relativePath, 'a notebook', onProgress);
  return createNotebook(subject, title, serializeCanvasDocument(notebookFromPages(sizes, images)), targetDir);
}
