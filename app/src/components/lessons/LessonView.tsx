import { useCallback, useEffect } from "react";
import { useSearchParams } from "react-router-dom";
import { FileViewer } from "@/components/files/FileViewer";
import type { DbFile } from "@/lib/db";
import { defaultSlide, slideFiles, slideKey, type LessonIndex } from "@/lib/ed/lessons";
import { markEdViewed } from "@/lib/ed/views";
import { openFileSmart, recordFileAccess } from "@/lib/files/openFile";
import { LessonSidebar } from "./LessonSidebar";
import { WorkspacePane } from "./workspace/WorkspacePane";

/**
 * An Ed lesson as Ed shows it: its slides down the left, the selected one on
 * the right — a PDF slide in the PDF viewer, a `code` challenge as its
 * workspace (brief and editor), any other as its markdown. The
 * slide rides in the URL (`&slide=<id>`), so a restored tab or Back reopens
 * it; without one the lesson opens on its first slide not yet completed.
 */
export function LessonView({
  file,
  index,
  files,
  title,
}: {
  /** The lesson's index file. */
  file: DbFile;
  index: LessonIndex;
  /** The subject's files, to find each slide's rows. */
  files: DbFile[];
  title: string;
}) {
  const [params, setParams] = useSearchParams();
  const wanted = params.get("slide");
  const found = wanted == null ? -1 : index.slides.findIndex((s, i) => slideKey(s, i) === wanted);
  const fallback = defaultSlide(index.slides);
  const selected = found >= 0 ? found : fallback ? index.slides.indexOf(fallback) : 0;
  const slide = index.slides[selected];

  const select = useCallback((position: number) => {
    setParams((prev) => {
      const next = new URLSearchParams(prev);
      next.set("slide", slideKey(index.slides[position], position));
      return next;
    }, { replace: true });
  }, [index.slides, setParams]);

  const { md, pdf } = slideFiles(file, slide, files);
  // A PDF slide's PDF until it is synced falls back to the slide's own page,
  // which links to it.
  const shown = (slide.pdf ? pdf : undefined) ?? md;
  // Only `code` challenges edit here; jupyter and the rest stay read-only.
  const workspace = slide.type === "code" && !slide.pdf ? slide.challengeId : null;

  // Each slide opened clears its never-opened mark, as a file page does.
  const mdId = md?.id;
  const pdfId = pdf?.id;
  useEffect(() => {
    for (const id of [mdId, pdfId]) {
      if (id != null) recordFileAccess({ id, subject_id: file.subject_id });
    }
  }, [mdId, pdfId, file.subject_id]);
  // …and tells Ed, which marks the slide seen.
  const slideId = slide.id;
  useEffect(() => {
    if (slideId != null) markEdViewed("slide", slideId);
  }, [slideId]);

  return (
    <div className="flex min-h-0 flex-1">
      <LessonSidebar
        title={title}
        index={index}
        selected={selected}
        onSelect={select}
        edUrl={slide.url ?? index.url}
      />
      <div className="flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden">
        {workspace != null && md ? (
          <WorkspacePane key={workspace} challengeId={workspace} brief={md} files={files} />
        ) : shown ? (
          /* In-slide links open the linked file beside this page. */
          <FileViewer key={shown.id} file={shown} files={files} onOpenFile={openFileSmart} />
        ) : (
          <div className="flex h-full flex-col items-center justify-center gap-1 px-6 text-center">
            <p className="text-sm text-muted-foreground">Not synced yet</p>
            <p className="text-xs text-muted-foreground/70">The next sync fetches this slide.</p>
          </div>
        )}
      </div>
    </div>
  );
}
