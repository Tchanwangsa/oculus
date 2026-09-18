import { convertFileSrc, invoke } from "@tauri-apps/api/core";

/**
 * Pictures on their way into a message.
 *
 * A CLI agent reads files, so an image has to be one before it can be talked
 * about: pasting or dropping one writes it into the library's
 * `agents/attachments/` and puts the path it comes back as into the message,
 * exactly as the `@` menu puts a course file's path there
 * (`app/src-tauri/src/harness/attach.rs`, `docs/harness.md`).
 *
 * Nothing is written while it is only *pending*. The composer holds the
 * clipboard's `File` or the dropped path and draws it from memory; the write
 * happens on send, so a picture pasted and then thought better of leaves
 * nothing behind on disk to sweep up.
 */
export interface PendingAttachment {
  id: string;
  /** For the alt text and the tooltip — never for the filename on disk,
   *  which Rust names itself. */
  name: string;
  /** What the strip draws: a blob URL for pasted bytes, an asset URL for a
   *  file that is already on disk. */
  preview: string;
  source: { kind: "bytes"; file: File } | { kind: "path"; path: string };
}

/** Extensions a dropped file is worth offering to the agent. The bytes are
 *  sniffed in Rust regardless — this only keeps a dropped folder of PDFs from
 *  becoming eight refusals. */
const IMAGE_EXT = /\.(png|jpe?g|gif|webp|heic|heif|avif)$/i;

let seq = 0;
const nextId = () => `att-${Date.now()}-${seq++}`;

/** Every image in a clipboard or drop payload, in order. */
export function imageFiles(list: FileList | null | undefined): File[] {
  if (!list) return [];
  return Array.from(list).filter((f) => f.type.startsWith("image/"));
}

/** …and the same filter for a native drop, which hands over paths. */
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

/** Frees a blob URL. An asset URL has nothing to free. */
export function releaseAttachment(a: PendingAttachment): void {
  if (a.source.kind === "bytes") URL.revokeObjectURL(a.preview);
}

/** A `File` as base64, the form the IPC takes it in — a byte array would
 *  cross as a JSON list of numbers, some seven characters per byte. */
function base64(file: Blob): Promise<string> {
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

/**
 * Write one pending picture into the library, answering with the path the
 * agent opens it by — `./attachments/<name>`, relative to `agents/`, which is
 * every thread's working directory.
 */
export async function writeAttachment(a: PendingAttachment): Promise<string> {
  if (a.source.kind === "path") {
    return invoke<string>("harness_attach_file", { path: a.source.path });
  }
  return invoke<string>("harness_attach_image", { data: await base64(a.source.file) });
}

/**
 * The same path as the webview sees it: `agents/attachments/<name>`, under the
 * data directory.
 *
 * Both spellings are accepted because both are written — the composer sends
 * the agent-relative one, and an agent quoting a picture back may write either
 * — and neither is a *library* path in the `courses/` sense, which is why this
 * is a matcher of its own rather than a case inside `libraryPath`.
 */
const ATTACHMENT_PATH = /^(?:\.\/)?(?:\.\.\/)?(?:agents\/)?attachments\/([A-Za-z0-9._-]+)$/;

/** The data-directory-relative path a fenced attachment stands for, or null. */
export function attachmentPath(raw: string): string | null {
  const m = ATTACHMENT_PATH.exec(raw.trim());
  return m ? `agents/attachments/${m[1]}` : null;
}

/** What an `<img>` in a bubble loads. Empty until the data directory has
 *  been asked for, which is one IPC call for the whole app
 *  (`app/src/hooks/useDataDir.ts`). */
export function attachmentSrc(dataDir: string, path: string): string {
  if (!dataDir) return "";
  return convertFileSrc(`${dataDir}/${path}`.replace(/\/{2,}/g, "/"));
}
