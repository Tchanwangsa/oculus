/** Only embedded raster images are allowed: SVG and external URLs can reference other resources. */
export function isCanvasImageDataUrl(value: unknown): value is string {
  if (typeof value !== 'string') return false;
  const match = /^data:image\/(png|jpeg|webp|gif);base64,/.exec(value);
  if (!match) return false;
  const bytes = value.slice(match[0].length);
  if (bytes.length < 16 || bytes.length % 4 !== 0 || !/^[A-Za-z0-9+/]+={0,2}$/.test(bytes)) return false;
  let head: string;
  try { head = atob(bytes.slice(0, 32)); } catch { return false; }
  if (match[1] === 'png') return head.startsWith('\x89PNG\r\n\x1a\n');
  if (match[1] === 'jpeg') return head.startsWith('\xff\xd8\xff');
  if (match[1] === 'gif') return head.startsWith('GIF87a') || head.startsWith('GIF89a');
  return head.startsWith('RIFF') && head.slice(8, 12) === 'WEBP';
}
