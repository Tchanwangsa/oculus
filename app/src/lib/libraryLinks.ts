import { convertFileSrc } from "@tauri-apps/api/core";

import type { DbFile } from "@/lib/db";

/**
 * Links and pictures inside a library file, resolved against the subject's
 * files: shared by `FileViewer`'s markdown and the document editor.
 */

/** The library file a link points at: a `../` path, or a raw Canvas
 *  `/files/<id>` or `/pages/<slug>` URL. Undefined for anything else. */
export function libraryLinkTarget(href: string, files: DbFile[]): DbFile | undefined {
  if (/^\.\.\//.test(href)) {
    const rel = decodeLinkPath(href.replace(/^\.\.\//, ""));
    return files.find((f) => f.relative_path.endsWith(rel));
  }
  if (href.includes("/courses/")) {
    const fileId = /\/files\/(\d+)/.exec(href)?.[1];
    if (fileId) return files.find((f) => f.canvas_id === Number(fileId));
    const slug = /\/pages\/([^/?#]+)/.exec(href)?.[1];
    if (slug) {
      return (
        files.find((f) => f.relative_path.endsWith(`pages/${slug}.md`)) ??
        // A renamed page keeps its Canvas URL slug while the saved file
        // tracks the title — source_url holds the canonical URL.
        files.find((f) => f.source_url?.endsWith(`/pages/${slug}`))
      );
    }
  }
  return undefined;
}

/** Link destinations are percent-encoded: markdown renderers encode them
 *  anyway, and a bare space or `(` would end the destination. Decoded per
 *  segment, so this never throws on a stray `%`. */
function decodeLinkPath(path: string): string {
  if (!path.includes("%")) return path;
  return path
    .split("/")
    .map((s) => {
      try {
        return decodeURIComponent(s);
      } catch {
        return s;
      }
    })
    .join("/");
}

/** The `../` link from the note at `notePath` (`courses/<code>/documents/…`)
 *  to a file of the same subject, which `libraryLinkTarget` resolves back.
 *  Null for a file under another subject. */
export function libraryLinkHref(notePath: string, targetPath: string): string | null {
  const subject = /^courses\/[^/]+\//.exec(notePath)?.[0];
  if (!subject || !targetPath.startsWith(subject)) return null;
  const rest = targetPath
    .slice(subject.length)
    .split("/")
    .map((s) => encodeURIComponent(s).replace(/[()]/g, (c) => `%${c.charCodeAt(0).toString(16).toUpperCase()}`))
    .join("/");
  return `../${rest}`;
}

/** What an `<img>` loads for `src` in the file at `relativePath`: a schemeless
 *  path resolves against that file's folder; anything with a scheme is as-is.
 *  Empty until the data dir is known. */
export function libraryImageSrc(src: string, relativePath: string, dataDir: string): string {
  if (!src || /^(https?:|data:|asset:|blob:)/.test(src)) return src;
  if (!dataDir) return "";
  const baseDir = relativePath.replace(/[^/]+$/, "");
  return convertFileSrc(`${dataDir}/${baseDir}${src}`.replace(/\/{2,}/g, "/"));
}
