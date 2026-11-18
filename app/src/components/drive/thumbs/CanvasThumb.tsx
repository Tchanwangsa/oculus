import { useEffect, useRef, useState } from "react";
import { PAGE_COLOR } from "@/lib/canvas/renderPaper";
import { cachedCover, cachedDrawnPicture, coverIsFresh, drawnPicture, readCover, type DrawnPicture } from "./drawings";
import type { FileStamp } from "./pictureCache";

/**
 * A drawing's picture. A notebook shows the first-page picture its editor
 * writes for chat agents, read again when the kept one may predate the last
 * save (`coverIsFresh`); one never opened has none, and it and a `.canvas`
 * are rendered from the file (`drawnPicture`). A kept picture shows in the
 * first frame. Calls `onFail` for an empty canvas or one that can't be drawn.
 */
export function CanvasThumb({ file, notebook, fileId, onFail }: {
  file: FileStamp;
  notebook: boolean;
  /** The notebook's `files` row, whose cover is looked up first. */
  fileId?: number;
  onFail: () => void;
}) {
  const [picture, setPicture] = useState<DrawnPicture | null>(() => cachedDrawnPicture(file, notebook, fileId));
  const fail = useRef(onFail);
  fail.current = onFail;
  const { path, size, modified } = file;

  useEffect(() => {
    let live = true;
    const settle = (next: DrawnPicture | null) => {
      if (!live) return;
      if (next) setPicture(next);
      else fail.current();
    };
    const draw = () => drawnPicture({ path, size, modified }, notebook, () => live);
    let work: Promise<DrawnPicture | null>;
    if (notebook && fileId != null) {
      const kept = cachedCover(fileId);
      if (kept && coverIsFresh(kept, modified)) return () => { live = false; };
      work = readCover(fileId)
        .catch(() => null)
        .then((cover) => (cover ? { url: cover.url, background: PAGE_COLOR } : live ? draw() : null));
    } else {
      work = draw();
    }
    work.then(settle, () => { if (live) fail.current(); });
    return () => { live = false; };
  }, [path, size, modified, notebook, fileId]);

  if (!picture) return null;
  return (
    <img
      src={picture.url}
      alt=""
      draggable={false}
      onError={onFail}
      style={{ backgroundColor: picture.background }}
      className="h-full w-full object-contain"
    />
  );
}
