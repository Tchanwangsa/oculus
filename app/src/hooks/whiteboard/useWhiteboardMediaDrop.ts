import { useEffect, useRef, useState, type DragEvent, type ClipboardEvent, type RefObject } from 'react';
import { readWhiteboardImages, type WhiteboardImportedImage } from '@/lib/whiteboard/importMedia';
import { placeImagesIn } from '@/lib/whiteboard/mediaPlacement';
import { screenToWorld } from '@/lib/whiteboard/geometry';
import type { WhiteboardDocument, Point } from '@/lib/whiteboard/model';
import { kindNoun } from '@/lib/whiteboard/kindWords';
import { useWhiteboardPlatform } from '@/lib/whiteboard/platform';

const imageFiles = (data: DataTransfer) => Array.from(data.files).filter(file => file.type.startsWith('image/') || /\.(png|jpe?g|gif|webp)$/i.test(file.name));
const imageUrl = (data: DataTransfer) => {
  const uri = data.getData('text/uri-list').split(/\r?\n/).find(line => line && !line.startsWith('#'));
  const html = data.getData('text/html');
  const src = uri || (html ? new DOMParser().parseFromString(html, 'text/html').querySelector('img')?.getAttribute('src') : '');
  // Only embedded pictures and the app's local asset server can be imported by URL.
  return src && /^(data:image\/(png|jpeg|webp|gif);base64,|asset:\/\/|https?:\/\/asset\.localhost\/)/i.test(src) ? src : null;
};

/**
 * Drops and pastes use the modal's decoder, placement and one-step insertion; a `notebook` previews them where its placement
 * puts them. `area` is the drawing, whose box drops are measured in; `root` takes focus after one.
 */
export function useWhiteboardMediaDrop(root: RefObject<HTMLDivElement | null>, area: RefObject<HTMLElement | null>, documentRef: RefObject<WhiteboardDocument>, insert: (images: WhiteboardImportedImage[], at?: Point) => void, notebook = false) {
  const platform = useWhiteboardPlatform();
  const [preview, setPreview] = useState<{ at: Point; images: WhiteboardImportedImage[] } | null>(null);
  const [error, setError] = useState<string | null>(null);
  const latest = useRef({ insert, platform });
  latest.current = { insert, platform };
  const abort = useRef<AbortController | null>(null);
  const native = useRef<{ key: string; promise: Promise<WhiteboardImportedImage[]> } | null>(null);
  const hover = useRef<Point | null>(null);
  const revision = useRef(0);
  useEffect(() => () => { revision.current++; hover.current = null; native.current = null; abort.current?.abort(); }, []);
  const world = (at: Point) => screenToWorld(at, documentRef.current.viewport);
  const read = async (files: File[], signal: AbortSignal) => readWhiteboardImages(files, signal);
  const finish = async (promise: Promise<WhiteboardImportedImage[]>, at?: Point) => {
    const current = ++revision.current;
    setPreview(null); setError(null);
    try {
      const images = await promise;
      if (revision.current !== current) return;
      latest.current.insert(images, at);
    } catch (reason) {
      if (revision.current === current && !(reason instanceof DOMException && reason.name === 'AbortError')) setError(String(reason instanceof Error ? reason.message : reason));
    }
  };
  useEffect(() => {
    const target = area.current;
    if (!target || !platform.watchFileDrop || !platform.readDroppedImages) return;
    const prepare = (paths: string[]) => {
      const key = JSON.stringify(paths);
      if (native.current?.key !== key) {
        abort.current?.abort(); abort.current = new AbortController();
        const promise = latest.current.platform.readDroppedImages!(paths, abort.current.signal);
        native.current = { key, promise };
        // Attach a rejection handler while the user is still dragging.
        void promise.catch(() => {});
      }
      return native.current.promise;
    };
    return platform.watchFileDrop(target, (paths, at) => {
      hover.current = at;
      if (!at) { setPreview(null); return; }
      const key = JSON.stringify(paths);
      setPreview(current => ({ at, images: native.current?.key === key ? current?.images ?? [] : [] }));
      const promise = prepare(paths);
      void promise.then(images => {
        if (hover.current && native.current?.promise === promise) setPreview({ at: hover.current, images });
      }).catch(() => {});
    }, (paths, at) => {
      hover.current = null;
      window.getSelection()?.removeAllRanges(); root.current?.focus({ preventScroll: true });
      void finish(prepare(paths), world(at));
      native.current = null;
    });
  }, [root, area, platform]);
  const position = (event: DragEvent<HTMLCanvasElement>) => {
    const rect = event.currentTarget.getBoundingClientRect();
    return { x: event.clientX - rect.left, y: event.clientY - rect.top };
  };
  const accepts = (data: DataTransfer) => data.types.includes('Files') || data.types.includes('text/uri-list') || data.types.includes('text/html');
  const dragOver = (event: DragEvent<HTMLCanvasElement>) => {
    if (!accepts(event.dataTransfer)) return;
    event.preventDefault(); event.stopPropagation(); event.dataTransfer.dropEffect = 'copy';
    const at = position(event);
    setPreview(current => ({ at, images: current?.images ?? [] }));
  };
  const drop = (event: DragEvent<HTMLCanvasElement>) => {
    if (!accepts(event.dataTransfer)) return;
    event.preventDefault(); event.stopPropagation(); setPreview(null);
    window.getSelection()?.removeAllRanges(); root.current?.focus({ preventScroll: true });
    const files = imageFiles(event.dataTransfer), url = files.length ? null : imageUrl(event.dataTransfer);
    abort.current?.abort(); abort.current = new AbortController();
    const signal = abort.current.signal;
    const promise = files.length ? read(files, signal) : url ? fetch(url, { signal }).then(async response => {
      if (!response.ok) throw new Error('This image could not be read');
      const blob = await response.blob();
      return read([new File([blob], 'Dropped image', { type: blob.type })], signal);
    }) : Promise.reject(new Error(`Drop PNG, JPEG, WebP or GIF images onto the ${kindNoun(notebook ? 'notebook' : 'whiteboard')}`));
    void finish(promise, world(position(event)));
  };
  const paste = (event: ClipboardEvent<HTMLDivElement>) => {
    if ((event.target as HTMLElement).closest('input,textarea,[contenteditable=true]')) return;
    const files = imageFiles(event.clipboardData);
    if (!files.length) return;
    event.preventDefault(); event.stopPropagation();
    abort.current?.abort(); abort.current = new AbortController();
    void finish(read(files, abort.current.signal));
  };
  const rect = area.current?.getBoundingClientRect();
  const boxes = preview?.images.length && rect ? placeImagesIn(documentRef.current, notebook, preview.images, rect, world(preview.at)) : [];
  return { preview, boxes, error, clearError: () => setError(null), dragOver, drop, paste, dragLeave: () => setPreview(null) };
}
