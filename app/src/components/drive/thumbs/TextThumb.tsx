import { useEffect, useRef, useState } from "react";
import { readCourseFile } from "@/lib/files/courseFiles";
import { textPreview, type PreviewFormat, type PreviewLine } from "@/lib/drive/textPreview";
import { createJobQueue, PictureCache, stampKey, type FileStamp } from "./pictureCache";

/** `read_course_file` sends the whole file, so a bigger one keeps its glyph. */
export const MAX_TEXT_BYTES = 1024 * 1024;

const read = createJobQueue(2);
const previews = new PictureCache<PreviewLine[] | null>(300);

/** The kept preview for `file`: lines, null for nothing to show, undefined
 *  when it has not been read yet. */
export const cachedPreview = (file: FileStamp) => previews.peek(stampKey(file));

function loadPreview(file: FileStamp, format: PreviewFormat, live: () => boolean) {
  return previews.load(stampKey(file), (anyLive) => read(async () => {
    const lines = textPreview(await readCourseFile(file.path), format);
    return lines.length ? lines : null;
  }, anyLive), live);
}

/** Sized in the page's own width (`cqw`), so a wider card scales the page
 *  rather than re-wrapping it: at a 160px card the body is 7px. Ink is fixed
 *  dark: the sheet is white paper in either theme. */
const STYLE: Record<PreviewLine["style"], string> = {
  h1: "font-display text-[6.25cqw] font-semibold leading-tight text-neutral-900 mt-[1.9cqw] mb-[1.25cqw]",
  h2: "font-display text-[5.3cqw] font-semibold leading-tight text-neutral-900 mt-[1.9cqw] mb-[0.6cqw]",
  h3: "font-display text-[4.7cqw] font-semibold leading-snug text-neutral-900 mt-[1.25cqw]",
  body: "text-[4.4cqw] leading-[1.45] text-neutral-700",
  bullet: "flex gap-[1.9cqw] text-[4.4cqw] leading-[1.45] text-neutral-700",
  quote: "border-l border-neutral-300 pl-[2.5cqw] text-[4.4cqw] leading-[1.45] text-neutral-500",
  code: "font-mono text-[4.05cqw] leading-[1.45] whitespace-pre text-neutral-700",
  gap: "h-[3.1cqw]",
};

/**
 * The start of a text file on a white page, as Google Drive draws one:
 * markdown lightly rendered, other text as written, code in the code font.
 * Calls `onFail` when the file can't be read or has nothing to show.
 */
export function TextThumb({ file, format, onFail }: { file: FileStamp; format: PreviewFormat; onFail: () => void }) {
  const [lines, setLines] = useState<PreviewLine[] | null>(() => cachedPreview(file) ?? null);
  const fail = useRef(onFail);
  fail.current = onFail;
  const key = stampKey(file);

  useEffect(() => {
    let live = true;
    loadPreview(file, format, () => live)
      .then((next) => {
        if (!live) return;
        if (next) setLines(next);
        else fail.current();
      })
      .catch(() => { if (live) fail.current(); });
    return () => { live = false; };
    // `key` names the file; the stamp object is rebuilt every render.
  }, [key, format]);

  if (!lines) return null;
  return (
    <div aria-hidden className="@container h-full w-full overflow-hidden bg-white px-[9%] pt-[7%] break-words">
      {lines.map((line, i) => line.style === "bullet" ? (
        <div key={i} className={STYLE.bullet} style={{ paddingLeft: `${(line.depth ?? 0) * 3.75}cqw` }}>
          <span className="shrink-0 tabular-nums text-neutral-500">{line.marker}</span>
          <span className="min-w-0">{line.text}</span>
        </div>
      ) : (
        <div key={i} className={STYLE[line.style]}>{line.text}</div>
      ))}
    </div>
  );
}
