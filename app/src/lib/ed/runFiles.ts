import { decodeBase64 } from "@/lib/ed/termOutput";
import type { RunFile } from "@/lib/ed/workspace";

/**
 * How the Run tab shows a file the program left (`runFiles`): a picture
 * inline, text in a dialog, anything else (or too big to send inline) in
 * the file manager.
 */
export type RunFileView = "image" | "text" | "reveal";

export function runFileView(file: RunFile): RunFileView {
  if (file.dataUrl == null) return "reveal";
  if (file.mime.startsWith("image/")) return "image";
  return "text";
}

/** A `data:` URL's payload as UTF-8 text; null when it is not one. */
export function dataUrlText(url: string): string | null {
  const comma = url.indexOf(",");
  if (!url.startsWith("data:") || comma < 0) return null;
  const meta = url.slice(5, comma);
  const payload = url.slice(comma + 1);
  if (!meta.endsWith(";base64")) return decodeURIComponent(payload);
  return new TextDecoder().decode(decodeBase64(payload));
}
