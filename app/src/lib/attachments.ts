import { convertFileSrc, invoke } from "@tauri-apps/api/core";

/**
 * A picture on its way into a message. A CLI agent reads files, so on send it
 * is written to `agents/attachments/` and its path goes into the message (see
 * `docs/harness.md`). Until then it lives only in memory, so a discarded paste
 * leaves nothing on disk.
 */
export interface PendingAttachment {
  id: string;
  /** Alt text and tooltip only; Rust names the file on disk. */
  name: string;
  /** A blob URL for pasted bytes, an asset URL for a file on disk. */
  preview: string;
  source: { kind: "bytes"; file: File } | { kind: "path"; path: string };
}

/** Picture extensions, for a drop's paths and the open panel's filter; Rust
 *  still sniffs the bytes. */
export const IMAGE_EXTENSIONS = ["png", "jpg", "jpeg", "gif", "webp", "heic", "heif", "avif"];
const IMAGE_EXT = new RegExp(`\\.(${IMAGE_EXTENSIONS.join("|")})$`, "i");

let seq = 0;
const nextId = () => `att-${Date.now()}-${seq++}`;

/** Every image in a clipboard or drop payload, in order. */
export function imageFiles(list: FileList | null | undefined): File[] {
  if (!list) return [];
  return Array.from(list).filter((f) => f.type.startsWith("image/"));
}

/** The same filter for a native drop, which hands over paths. */
export function imagePaths(paths: string[]): string[] {
  return paths.filter((p) => IMAGE_EXT.test(p));
}

export function pendingFromFile(file: File): PendingAttachment {
  return {
    id: nextId(),
    name: file.name || "Pasted image",
    preview: URL.createObjectURL(file),
    source: { kind: "bytes", file },
  };
}

export function pendingFromPath(path: string): PendingAttachment {
  return {
    id: nextId(),
    name: path.slice(path.lastIndexOf("/") + 1),
    preview: convertFileSrc(path),
    source: { kind: "path", path },
  };
}

export function releaseAttachment(a: PendingAttachment): void {
  if (a.source.kind === "bytes") URL.revokeObjectURL(a.preview);
}

/** A `File` as base64 for IPC — a byte array would cross as a JSON list of
 *  numbers. */
export function base64(file: Blob): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => {
      const url = String(reader.result);
      resolve(url.slice(url.indexOf(",") + 1));
    };
    reader.onerror = () => reject(reader.error ?? new Error("could not read the image"));
    reader.readAsDataURL(file);
  });
}

/** Write one pending picture, returning `./attachments/<name>` — relative to
 *  `agents/`, every thread's working directory. */
export async function writeAttachment(a: PendingAttachment): Promise<string> {
  if (a.source.kind === "path") {
    return invoke<string>("harness_attach_file", { path: a.source.path });
  }
  return invoke<string>("harness_attach_image", { data: await base64(a.source.file) });
}

/** An attachment path in either spelling agents write (agent- or
 *  data-dir-relative); not a `courses/` path, so not part of `libraryPath`. */
const ATTACHMENT_PATH = /^(?:\.\/)?(?:\.\.\/)?(?:agents\/)?attachments\/([A-Za-z0-9._-]+)$/;

/** The data-directory-relative path a fenced attachment stands for, or null. */
export function attachmentPath(raw: string): string | null {
  const m = ATTACHMENT_PATH.exec(raw.trim());
  return m ? `agents/attachments/${m[1]}` : null;
}

/** What an `<img>` in a bubble loads; empty until `useDataDir` resolves. */
export function attachmentSrc(dataDir: string, path: string): string {
  if (!dataDir) return "";
  return convertFileSrc(`${dataDir}/${path}`.replace(/\/{2,}/g, "/"));
}

/**
 * A long paste held beside the box as a card rather than poured into it. It
 * persists with the draft (`draftStore`), since it can be large and edited,
 * and goes into the message inline on send — a `<pasted_text>` block, no file.
 */
export interface PastedText {
  id: string;
  text: string;
}

/** A plain-text paste at either bound becomes a `PastedText`. */
export const LONG_PASTE = { chars: 1000, lines: 15 } as const;

/** Lines as the reader counts them: a trailing newline opens no line. */
export function lineCount(text: string): number {
  const t = text.replace(/\n+$/, "");
  return t ? t.split("\n").length : 0;
}

export function isLongPaste(text: string): boolean {
  return text.length >= LONG_PASTE.chars || lineCount(text) >= LONG_PASTE.lines;
}

export function pastedText(text: string): PastedText {
  return { id: nextId(), text: text.replace(/\r\n?/g, "\n") };
}

const PASTE_OPEN = "<pasted_text>\n";
const PASTE_CLOSE = "\n</pasted_text>";

/** A line that would read as a block's end; `splitPastedText` takes one
 *  backslash back off. */
const CLOSE_LINE = /^(\\*)<\/pasted_text>$/gm;
const ESCAPED_CLOSE_LINE = /^\\(\\*)<\/pasted_text>$/gm;

/** A pasted text as the message spells it. Blank edge lines are dropped (a
 *  first line keeps its indent); empty text has no block. A line that is
 *  exactly the closing tag gains a backslash, so the block's end is the first. */
export function pastedBlock(text: string): string {
  const body = text.replace(/^(?:[ \t]*\n)+/, "").trimEnd().replace(CLOSE_LINE, "\\$1</pasted_text>");
  return body ? `${PASTE_OPEN}${body}${PASTE_CLOSE}` : "";
}

/** The typed text, then each pasted text's block, blank-line separated. */
export function withPastedText(typed: string, pasted: string[]): string {
  return [typed.trim(), ...pasted.map(pastedBlock)].filter(Boolean).join("\n\n");
}

/**
 * `withPastedText` undone: the pasted texts, and the rest with their gaps
 * closed. A block opens at the start or after a blank line and ends at the
 * first closing tag alone on its line; `pastedBlock` escapes any such line in
 * the text, so the two round-trip exactly.
 */
export function splitPastedText(message: string): { text: string; pasted: string[] } {
  const pasted: string[] = [];
  const kept: string[] = [];
  let at = 0;
  for (;;) {
    let open = message.indexOf(PASTE_OPEN, at);
    while (open > 0 && message.slice(open - 2, open) !== "\n\n") {
      open = message.indexOf(PASTE_OPEN, open + 1);
    }
    if (open < 0) break;
    const body = open + PASTE_OPEN.length;
    // Alone on its line: the end of the message or a newline follows.
    let close = message.indexOf(PASTE_CLOSE, body);
    while (close >= 0 && (message[close + PASTE_CLOSE.length] ?? "\n") !== "\n") {
      close = message.indexOf(PASTE_CLOSE, close + 1);
    }
    if (close < 0) break;
    kept.push(message.slice(at, open));
    pasted.push(message.slice(body, close).replace(ESCAPED_CLOSE_LINE, "$1</pasted_text>"));
    at = close + PASTE_CLOSE.length;
  }
  if (!pasted.length) return { text: message, pasted };
  kept.push(message.slice(at));
  const text = kept
    .map((k) => k.replace(/^\s*\n/, "").trimEnd())
    .filter((k) => k.trim())
    .join("\n\n");
  return { text, pasted };
}

/** The text, then the pictures' paths on their own line, backtick-fenced like
 *  a mention so `splitLibraryPaths` finds them. */
export function withAttachments(text: string, paths: string[]): string {
  return [text, paths.map((p) => `\`${p}\``).join(" ")].filter(Boolean).join("\n\n");
}
