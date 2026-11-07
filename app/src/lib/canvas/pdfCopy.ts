import { createCanvasDocument, serializeCanvasDocument, type CanvasDocument, type CanvasElement } from './model';
import { createCanvas } from './files';
import { openCanvasLibraryPdf } from './importPdf';
import { MAX_CANVAS_IMPORT_BYTES, MAX_CANVAS_IMPORT_IMAGES, type CanvasImportedImage } from './importMedia';
import { secureId } from './ids';
import type { DbFile } from '@/lib/db';

type Subject = { id: number; code: string };
export type PdfPageSize = { width: number; height: number };

/**
 * Every page of a library PDF rendered as for any PDF import, with each page's size in points. The
 * PDF is only read. Over 100 pages, or pages coming to over 64 MB, is refused with a sentence naming
 * `into` ("a notebook", "a canvas").
 */
export async function readPdfPages(
  relativePath: string, into: string, onProgress?: (done: number, total: number) => void,
): Promise<{ sizes: PdfPageSize[]; images: CanvasImportedImage[] }> {
  const pdf = await openCanvasLibraryPdf(relativePath);
  try {
    const total = pdf.pages.length;
    if (total > MAX_CANVAS_IMPORT_IMAGES) throw new Error(`This PDF has ${total} pages; ${into} can start from up to ${MAX_CANVAS_IMPORT_IMAGES}.`);
    const images: CanvasImportedImage[] = [];
    let bytes = 0;
    for (let page = 1; page <= total; page++) {
      onProgress?.(page - 1, total);
      const image = await pdf.renderPage(page);
      bytes += image.dataUrl.length;
      if (bytes > MAX_CANVAS_IMPORT_BYTES) throw new Error(`This PDF is too large to start ${into} from: its pages come to more than 64 MB.`);
      images.push(image);
    }
    onProgress?.(total, total);
    return { sizes: pdf.pages, images };
  } finally {
    void pdf.close().catch(() => {});
  }
}

/** Where the first page's top-left corner sits, so it is in view at the default camera. */
const BOARD_MARGIN = 48;
/** The space between two pages, as between images in one import. */
const BOARD_GAP = 24;

/**
 * A canvas (no paper) holding a PDF's pages as ordinary images at the PDF page size, top to bottom
 * in page order with a gap, for sketching around. Nothing is locked: the pages move like any image.
 */
export function boardFromPages(sizes: readonly PdfPageSize[], images: readonly CanvasImportedImage[]): CanvasDocument {
  if (!sizes.length || sizes.length !== images.length) throw new Error('A canvas copy needs one picture for each page.');
  let top = BOARD_MARGIN;
  const elements: CanvasElement[] = sizes.map((size, i) => {
    const y = top;
    top += size.height + BOARD_GAP;
    return {
      id: secureId(), type: 'image', x: BOARD_MARGIN, y, width: size.width, height: size.height,
      dataUrl: images[i].dataUrl, sourceName: images[i].name, naturalWidth: images[i].width, naturalHeight: images[i].height,
      stroke: 'transparent', fill: 'transparent', strokeWidth: 0, style: 'clean', seed: 0,
    };
  });
  return { ...createCanvasDocument(), elements };
}

/** A new canvas with a copy of a library PDF's pages laid out on it; the PDF itself is never changed. */
export async function createCanvasFromPdf(
  subject: Subject, relativePath: string, title: string, onProgress?: (done: number, total: number) => void,
): Promise<DbFile> {
  const { sizes, images } = await readPdfPages(relativePath, 'a canvas', onProgress);
  return createCanvas(subject, title, serializeCanvasDocument(boardFromPages(sizes, images)));
}
