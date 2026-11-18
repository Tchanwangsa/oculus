import { useEffect, useRef, useState } from "react";
import { openPdf } from "@/lib/pdf/pdfView";
import { renderPage } from "@/lib/pdf/pdfRender";
import { PICTURE_WIDTH, PictureCache, Skipped, stampKey, type FileStamp } from "./pictureCache";

/** About 60 KB a page as PNG, so a few MB in all. */
const pages = new PictureCache<string>(150, (url) => URL.revokeObjectURL(url));

/** The kept first page for `file`, if it has been drawn. */
export const cachedPdfPicture = (file: FileStamp) => pages.peek(stampKey(file));

/** Rasterises page 1 of `path` and keeps it as a PNG blob URL under `file`. */
function pdfPicture(file: FileStamp, path: string, live: () => boolean): Promise<string> {
  return pages.load(stampKey(file), async (anyLive) => {
    const doc = openPdf(path);
    try {
      const first = (await doc.pages)[0];
      if (!first) throw new Error("no pages");
      const width = PICTURE_WIDTH;
      const height = Math.max(1, Math.round((PICTURE_WIDTH * first.height) / first.width));
      const image = await renderPage(path, 1, width, height, anyLive);
      if (!image) throw new Skipped();
      const canvas = document.createElement("canvas");
      canvas.width = width;
      canvas.height = height;
      canvas.getContext("2d")?.putImageData(image, 0, 0);
      const blob = await new Promise<Blob | null>((done) => canvas.toBlob(done, "image/png"));
      if (!blob) throw new Error("encode failed");
      return URL.createObjectURL(blob);
    } finally {
      doc.release();
    }
  }, live);
}

/**
 * A PDF's first page, rasterised by the viewer's own renderer (`pdf_open`,
 * `pdf_render` through `openPdf` and `renderPage`'s queue) and kept until
 * the file changes, so a remounted card shows it in its first frame. `path`
 * is the PDF itself, or an Office file's converted copy; `file` stamps the
 * entry. Calls `onFail` when the PDF can't be opened.
 */
export function PdfThumb({ file, path, onFail }: { file: FileStamp; path: string; onFail: () => void }) {
  const [url, setUrl] = useState<string | null>(() => cachedPdfPicture(file) ?? null);
  const fail = useRef(onFail);
  fail.current = onFail;
  const key = stampKey(file);

  useEffect(() => {
    let live = true;
    pdfPicture(file, path, () => live).then(
      (next) => { if (live) setUrl(next); },
      () => { if (live) fail.current(); },
    );
    return () => { live = false; };
    // `key` names the file; the stamp object is rebuilt every render.
  }, [key, path]);

  if (!url) return null;
  return (
    <img
      src={url}
      alt=""
      draggable={false}
      onError={onFail}
      className="h-full w-full bg-white object-cover object-top"
    />
  );
}
