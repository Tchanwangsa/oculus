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

/** The text, then the pictures' paths on their own line, backtick-fenced like
 *  a mention so `splitLibraryPaths` finds them. */
export function withAttachments(text: string, paths: string[]): string {
  return [text, paths.map((p) => `\`${p}\``).join(" ")].filter(Boolean).join("\n\n");
}
