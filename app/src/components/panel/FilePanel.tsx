import { useEffect, useRef } from "react";
import { useSidePanelStore, type FileLocate } from "@/stores/sidePanelStore";
import { useActivePath } from "@/stores/tabStore";
import { useSubjectFiles } from "@/hooks/useSubjectFiles";
import { PanelHeader } from "@/components/panel/PanelHeader";
import { FileViewer, PdfMdToggle, usePdfMd } from "@/components/files/FileViewer";
import { MarkdownUnavailable } from "@/components/files/ParseState";
import { filePagePath, fileTitle, openFileSmart } from "@/lib/openFile";
import { recordRecent } from "@/lib/recents";
import { centerIn, matchText, scrollerOf } from "@/lib/locateQuote";
import type { DbFile } from "@/lib/db";

/** The CSS Custom Highlight a cited markdown passage is painted with
 *  (`::highlight(citation-hit)` in `index.css`). */
const HIGHLIGHT = "citation-hit";

/** A file open in the side panel; `SidePanel` owns the expand-to-page move. */
export default function FilePanel({
  file,
  locate,
  paneId,
  onExpand,
}: {
  file: DbFile;
  /** A cited spot (`openCitation`): a page in the PDF, or a passage of
   *  markdown to highlight. */
  locate?: FileLocate;
  paneId: number;
  onExpand: (path: string, newTab: boolean) => void;
}) {
  const { files } = useSubjectFiles(file.subject_id);
  const pdf = usePdfMd(file);
  const here = useActivePath();
  const close = useSidePanelStore((s) => s.close);
  const body = useRef<HTMLDivElement>(null);

  // Navigating the front tab out of the file's subject closes it. The page
  // it was opened on (a chat, another subject's lecture) doesn't count.
  const openedOn = useRef(here.split("?")[0]);
  useEffect(() => {
    if (here.split("?")[0] === openedOn.current) return;
    const m = /^\/subjects\/(\d+)/.exec(here);
    if (!m || m[1] !== String(file.subject_id)) close(paneId);
  }, [here, file.subject_id, close, paneId]);

  // A cited page is in the PDF, whichever face was showing.
  const { setViewMode } = pdf;
  useEffect(() => {
    if (locate?.page) setViewMode("pdf");
  }, [locate?.seq, locate?.page, setViewMode]);

  // A cited passage of markdown: highlighted and scrolled to once the text
  // has rendered (it loads async, hence the observer).
  const quote = locate && !locate.page ? locate.quote : undefined;
  useEffect(() => {
    const root = body.current;
    if (!quote || !root || !("highlights" in CSS)) return;
    const find = () => {
      const range = matchText(root, quote);
      const el = range?.startContainer.parentElement;
      if (!range || !el) return false;
      CSS.highlights.set(HIGHLIGHT, new Highlight(range));
      const scroller = scrollerOf(el);
      if (scroller) centerIn(scroller, el);
      return true;
    };
    if (find()) return () => CSS.highlights.delete(HIGHLIGHT);
    const watch = new MutationObserver(() => find() && watch.disconnect());
    watch.observe(root, { childList: true, subtree: true });
    return () => {
      watch.disconnect();
      CSS.highlights.delete(HIGHLIGHT);
    };
  }, [quote, locate?.seq]);

  // Feeds the "Recently visited" row on the subject home.
  useEffect(() => {
    recordRecent(file.subject_id, {
      kind: "file",
      ref: file.relative_path,
      title: fileTitle(file),
      category: file.category ?? undefined,
    });
  }, [file]);

  return (
    <>
      <PanelHeader
        title={fileTitle(file)}
        onExpand={(newTab) =>
          onExpand(filePagePath(file.subject_id, file.relative_path), newTab)
        }
        onClose={() => close(paneId)}
        actions={
          pdf.isPdf && pdf.mdChecked ? (
            pdf.mdExists ? (
              <PdfMdToggle value={pdf.viewMode} onChange={pdf.setViewMode} />
            ) : (
              <MarkdownUnavailable file={file} />
            )
          ) : undefined
        }
      />
      <div ref={body} className="flex-1 min-h-0 overflow-hidden flex flex-col">
        <FileViewer
          file={file}
          files={files}
          onOpenFile={openFileSmart}
          pdfViewMode={pdf.viewMode}
          locate={locate}
        />
      </div>
    </>
  );
}
