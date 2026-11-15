import { fullCitation, type Citation } from "@/lib/citations/parse";
import { attachmentPath, isImagePath } from "@/lib/harness/attachments";

/** Prose, a fenced library path, or an attachment: a picture (`image`) or a
 *  PDF or text file (`attachment`). Their `path` is data-directory-relative
 *  (`agents/attachments/…`, `<img>`-loadable through `attachmentSrc`); `raw`
 *  is the message's own spelling. See `app/src/lib/harness/attachments.ts`. */
export type TextPart =
  | { kind: "text"; text: string }
  | { kind: "path"; path: string; cite: Citation }
  | { kind: "image"; path: string; raw: string }
  | { kind: "attachment"; path: string; raw: string };

const FENCED = /`([^`\n]+)`/g;

/** Splits text into prose, backtick-fenced library paths and attachments —
 *  the one rule composer, bubble and copy share. Only a fence that is
 *  *wholly* a path counts; any other fence stays prose, backticks and all. */
export function splitLibraryPaths(text: string): TextPart[] {
  const parts: TextPart[] = [];
  let at = 0;
  for (const m of text.matchAll(FENCED)) {
    const attached = attachmentPath(m[1]);
    const cite = attached ? null : fullCitation(m[1]);
    const i = m.index ?? 0;
    if (!attached && !cite) continue;
    if (i > at) parts.push({ kind: "text", text: text.slice(at, i) });
    if (attached) parts.push({ kind: isImagePath(attached) ? "image" : "attachment", path: attached, raw: m[1] });
    else parts.push({ kind: "path", path: cite!.path, cite: cite! });
    at = i + m[0].length;
  }
  if (at < text.length) parts.push({ kind: "text", text: text.slice(at) });
  return parts;
}
