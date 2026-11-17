import { convertFileSrc } from "@tauri-apps/api/core";

import type { DbFile } from "@/lib/db";
import { PAGES_DIR } from "@/lib/files/libraryLayout";

/**
 * Links and pictures inside a library file, resolved against the subject's
 * files: shared by `FileViewer`'s markdown and the document editor.
 */

/** The library file a link points at: a `../` path, a path below the linking
 *  file's folder (`fromPath`; an Ed lesson's `<id>-<slug>/…` assets), or a raw
 *  Canvas `/files/<id>` or `/pages/<slug>` URL. Undefined for anything else. */
export function libraryLinkTarget(href: string, files: DbFile[], fromPath?: string): DbFile | undefined {
  if (/^\.\.\//.test(href)) {
    const exact = fromPath && resolveFrom(fromPath, decodeLinkPath(href));
    const hit = exact ? files.find((f) => f.relative_path === exact) : undefined;
    if (hit) return hit;
    const rel = decodeLinkPath(href.replace(/^(\.\.\/)+/, ""));
    return files.find((f) => f.relative_path.endsWith(`/${rel}`));
  }
  if (fromPath && /^[^/#?:]+\//.test(href)) {
    const rel = fromPath.replace(/[^/]+$/, "") + decodeLinkPath(href.replace(/^\.\//, ""));
    const hit = files.find((f) => f.relative_path === rel);
    if (hit) return hit;
  }
  if (href.includes("/courses/")) {
    const fileId = /\/files\/(\d+)/.exec(href)?.[1];
    if (fileId) return files.find((f) => f.canvas_id === Number(fileId));
    const slug = /\/pages\/([^/?#]+)/.exec(href)?.[1];
    if (slug) {
      return (
        files.find((f) => f.relative_path.endsWith(`${PAGES_DIR}/${slug}.md`)) ??
        // A renamed page keeps its Canvas URL slug while the saved file
        // tracks the title — source_url holds the canonical URL.
        files.find((f) => f.source_url?.endsWith(`/pages/${slug}`))
      );
    }
  }
  return undefined;
}

/** `href` (relative, `..` allowed) resolved against the folder of the file at
 *  `fromPath`; null when it climbs out of the data dir. */
function resolveFrom(fromPath: string, href: string): string | null {
  const parts = fromPath.split("/").slice(0, -1);
  for (const part of href.split("/")) {
    if (part === "..") {
      if (!parts.length) return null;
      parts.pop();
    } else if (part && part !== ".") parts.push(part);
  }
  return parts.join("/");
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

/** What an `<img>` loads for `src` in the file at `relativePath`: a schemeless
 *  path resolves against that file's folder; anything with a scheme is as-is.
 *  Empty until the data dir is known. */
export function libraryImageSrc(src: string, relativePath: string, dataDir: string): string {
  if (!src || /^(https?:|data:|asset:|blob:)/.test(src)) return src;
  if (!dataDir) return "";
  const baseDir = relativePath.replace(/[^/]+$/, "");
  return convertFileSrc(`${dataDir}/${baseDir}${src}`.replace(/\/{2,}/g, "/"));
}
