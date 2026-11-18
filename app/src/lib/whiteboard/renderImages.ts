import type { CanvasElement } from './model';
import { isCanvasImageDataUrl } from './imageSource';

interface ImageEntry {
  source: string;
  image: HTMLImageElement;
  ready: boolean;
  loaded: Promise<void>;
}

// Elements hold decoded images for their lifetime; sources share them across clones and edits.
const elements = new WeakMap<CanvasElement, ImageEntry>();
const sources = new Map<string, WeakRef<ImageEntry>>();
const listeners = new Set<() => void>();

/** Repaint subscribers when an image finishes decoding, including images loaded from saved files. */
export function subscribeCanvasImages(listener: () => void): () => void {
  listeners.add(listener);
  return () => { listeners.delete(listener); };
}

function imageEntry(element: CanvasElement): ImageEntry | undefined {
  const existing = elements.get(element);
  if (existing?.source === element.dataUrl) return existing;
  if (typeof element.dataUrl !== 'string' || typeof Image === 'undefined') return;
  const shared = sources.get(element.dataUrl)?.deref();
  if (shared) { elements.set(element, shared); return shared; }
  if (!isCanvasImageDataUrl(element.dataUrl)) return;
  const image = new Image();
  const entry: ImageEntry = { source: element.dataUrl, image, ready: false, loaded: Promise.resolve() };
  entry.loaded = new Promise<void>((resolve, reject) => {
    image.onload = () => {
      entry.ready = true;
      resolve();
      for (const listener of listeners) listener();
    };
    image.onerror = () => {
      reject(new Error(`Could not decode canvas image${element.sourceName ? ` “${element.sourceName}”` : ''}.`));
      for (const listener of listeners) listener();
    };
  });
  // Painting is synchronous; exports observe the same failure through loadCanvasImages.
  void entry.loaded.catch(() => {});
  elements.set(element, entry);
  for (const [source, ref] of sources) if (!ref.deref()) sources.delete(source);
  sources.set(element.dataUrl, new WeakRef(entry));
  image.src = element.dataUrl;
  return entry;
}

/** PNG export waits for decoded pixels, so a newly opened canvas never exports blank images. */
export async function loadCanvasImages(scene: readonly CanvasElement[]): Promise<void> {
  await Promise.all(scene.filter(element => element.type === 'image').map(element => {
    const entry = imageEntry(element);
    if (!entry) throw new Error('Could not load the embedded canvas image.');
    return entry.loaded;
  }));
}

/** Draws in the caller's local element coordinates and inherits its opacity. */
export function drawCanvasImage(context: CanvasRenderingContext2D, element: CanvasElement) {
  const entry = imageEntry(element);
  const x = Math.min(0, element.width), y = Math.min(0, element.height);
  const width = Math.abs(element.width), height = Math.abs(element.height);
  if (entry?.ready) context.drawImage(entry.image, x, y, width, height);
  else {
    context.save();
    context.fillStyle = '#e8e8eb'; context.fillRect(x, y, width, height);
    context.restore();
  }
}
