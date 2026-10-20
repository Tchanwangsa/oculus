export interface CanvasImportedImage { dataUrl: string; width: number; height: number; name: string }
export interface CanvasImportPdf {
  name: string;
  pages: { width: number; height: number }[];
  /** PDF page numbers start at 1. */
  renderPage(page: number, thumbnail?: boolean): Promise<CanvasImportedImage>;
  close(): Promise<void>;
}
export const CANVAS_IMAGE_ACCEPT = "image/png,image/jpeg,image/webp,image/gif";
export const MAX_CANVAS_IMPORT_BYTES = 64 * 1024 * 1024;
export const MAX_CANVAS_IMPORT_IMAGES = 100;
export function assertCanvasImportSize(images: CanvasImportedImage[]): void {
  if (images.length > MAX_CANVAS_IMPORT_IMAGES || images.reduce((sum, image) => sum + image.dataUrl.length, 0) > MAX_CANVAS_IMPORT_BYTES) {
    throw new Error("Add up to 100 images and 64 MB at a time. Choose fewer images or PDF pages.");
  }
}
const MAX_IMAGE_BYTES = 24 * 1024 * 1024;
function aborted(signal?: AbortSignal): void {
  if (signal?.aborted) throw new DOMException("Image import cancelled", "AbortError");
}
/** Detect raster content rather than trusting filenames or browser MIME types. */
export function canvasImageMime(bytes: Uint8Array): string | null {
  const starts = (...prefix: number[]) => prefix.every((value, i) => bytes[i] === value);
  if (starts(137, 80, 78, 71, 13, 10, 26, 10)) return "image/png";
  if (starts(255, 216, 255)) return "image/jpeg";
  const ascii = (at: number, value: string) => [...value].every((char, i) => bytes[at + i] === char.charCodeAt(0));
  if (ascii(0, "GIF87a") || ascii(0, "GIF89a")) return "image/gif";
  if (ascii(0, "RIFF") && ascii(8, "WEBP")) return "image/webp";
  return null;
}
export function asDataUrl(blob: Blob, signal?: AbortSignal): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    const abort = () => reader.abort();
    const cleanup = () => signal?.removeEventListener("abort", abort);
    reader.onload = () => { cleanup(); resolve(reader.result as string); };
    reader.onerror = () => { cleanup(); reject(new Error("Could not read this image")); };
    reader.onabort = () => { cleanup(); reject(new DOMException("Image import cancelled", "AbortError")); };
    signal?.addEventListener("abort", abort, { once: true });
    reader.readAsDataURL(blob);
    if (signal?.aborted) reader.abort();
  });
}
function dimensions(blob: Blob, signal?: AbortSignal): Promise<{ width: number; height: number }> {
  return new Promise((resolve, reject) => {
    const image = new Image();
    const url = URL.createObjectURL(blob);
    const cleanup = () => {
      image.onload = image.onerror = null;
      signal?.removeEventListener("abort", abort);
      URL.revokeObjectURL(url);
    };
    const abort = () => { cleanup(); image.src = ""; reject(new DOMException("Image import cancelled", "AbortError")); };
    image.onload = () => {
      const width = image.naturalWidth, height = image.naturalHeight;
      cleanup();
      if (width < 1 || height < 1 || width > 8192 || height > 8192 || width * height > 32_000_000) {
        reject(new Error("Choose an image up to 8192 pixels per side and 32 million pixels"));
      } else resolve({ width, height });
    };
    image.onerror = () => { cleanup(); reject(new Error("This image could not be decoded")); };
    signal?.addEventListener("abort", abort, { once: true });
    image.src = url;
    if (signal?.aborted) abort();
  });
}
/** Decode one file at a time to keep a multi-selection bounded in memory. */
export async function readCanvasImages(files: File[], signal?: AbortSignal): Promise<CanvasImportedImage[]> {
  if (files.length > MAX_CANVAS_IMPORT_IMAGES || files.reduce((sum, file) => sum + file.size, 0) > MAX_CANVAS_IMPORT_BYTES * 3 / 4) {
    throw new Error("Add up to 100 images and 64 MB at a time");
  }
  const images: CanvasImportedImage[] = [];
  for (const file of files) {
    aborted(signal);
    if (file.size === 0 || file.size > MAX_IMAGE_BYTES) throw new Error(`${file.name}: choose an image between 1 byte and 24 MB`);
    const mime = canvasImageMime(new Uint8Array(await file.slice(0, 16).arrayBuffer()));
    if (!mime) throw new Error(`${file.name}: choose a PNG, JPEG, WebP or GIF image`);
    const blob = new Blob([file], { type: mime });
    const size = await dimensions(blob, signal);
    aborted(signal);
    const dataUrl = await asDataUrl(blob, signal);
    aborted(signal);
    images.push({ dataUrl, ...size, name: file.name });
    assertCanvasImportSize(images);
  }
  return images;
}
export function canvasPdfRasterSize(width: number, height: number, thumbnail = false): { width: number; height: number } {
  if (!Number.isFinite(width) || !Number.isFinite(height) || width <= 0 || height <= 0) throw new Error("This PDF has an invalid page size");
  const side = thumbnail ? 240 : 2400;
  const scale = Math.min(side / width, side / height, Math.sqrt(4_000_000 / (width * height)));
  return { width: Math.max(1, Math.floor(width * scale)), height: Math.max(1, Math.floor(height * scale)) };
}
