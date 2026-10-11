import { useEffect, useState } from "react";
import { openPdf, type PdfPageSize } from "@/lib/pdf/pdfView";

/** Opens `path` for the life of the mount: its page sizes once Rust has
 *  them, or why it could not be opened. */
export function usePdfDocument(path: string) {
  const [sizes, setSizes] = useState<PdfPageSize[] | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  useEffect(() => {
    const doc = openPdf(path);
    let live = true;
    doc.pages.then(
      (pages) => {
        if (!live) return;
        if (pages.length) setSizes(pages);
        else setLoadError("invalid");
      },
      (err) => live && setLoadError(String(err)),
    );
    return () => {
      live = false;
      doc.release();
    };
  }, [path]);

  return { sizes, loadError };
}
