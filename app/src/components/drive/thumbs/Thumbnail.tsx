import { useEffect, useRef, useState } from "react";
import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { Play } from "@phosphor-icons/react";
import { useDataDir } from "@/hooks/backend/useDataDir";
import { docPdfRelPath } from "@/lib/files/fileTypes";
import { kindOf } from "@/lib/drive/filters";
import type { DriveItem } from "@/lib/drive/items";
import { previewFormat, type PreviewFormat } from "@/lib/drive/textPreview";
import { cn } from "@/lib/utils";
import { ItemIcon } from "../content/ItemIcon";
import { CanvasThumb } from "./CanvasThumb";
import { cachedDrawnPicture } from "./drawings";
import { cachedPdfPicture, PdfThumb } from "./PdfThumb";
import type { FileStamp } from "./pictureCache";
import { cachedPreview, MAX_TEXT_BYTES, TextThumb } from "./TextThumb";
import { useInView } from "./useInView";

/**
 * A card's picture, drawn by what already draws each kind: a PDF's (or an
 * Office file's converted PDF's) first page through the PDF viewer's
 * renderer, a drawing through the canvas renderer (`CanvasThumb`), the start
 * of a text file as a page (`TextThumb`), a recording's frame from
 * `lecture_thumbnail`, an image itself. Anything else, or a picture that
 * can't be had, is the kind's glyph. Nothing loads until the card is near
 * the screen; a picture already made shows in the first frame, so the
 * details panel and a remounted card never flash the glyph.
 */
export function Thumbnail({ item, className }: { item: DriveItem; className?: string }) {
  const ref = useRef<HTMLDivElement>(null);
  const seen = useInView(ref);
  // A failure is the file's as it was: once it changes, it is tried again.
  const version = item.type === "file" ? `${item.key}:${item.entry.size}:${item.entry.modified}` : item.key;
  const [failed, setFailed] = useState<string | null>(null);
  const source = failed === version ? null : sourceOf(item);
  const draw = source != null && (seen || isKept(source));
  return (
    <div ref={ref} className={cn("relative flex items-center justify-center overflow-hidden bg-surface", className)}>
      <ItemIcon item={item} size={28} className="opacity-70" />
      {draw ? (
        <div className="absolute inset-0">
          {/* Keyed: another file's picture starts from its own kept one. */}
          <Picture key={version} source={source} onFail={() => setFailed(version)} />
        </div>
      ) : null}
      {item.type === "file" && kindOf(item.entry.name) === "video" && (
        <span className="absolute flex size-8 items-center justify-center rounded-full bg-black/45 text-white">
          <Play size={14} weight="fill" aria-hidden />
        </span>
      )}
    </div>
  );
}

type Source =
  | { kind: "pdf"; path: string; file: FileStamp }
  | { kind: "image"; path: string }
  | { kind: "drawing"; file: FileStamp; notebook: boolean; fileId?: number }
  | { kind: "text"; file: FileStamp; format: PreviewFormat }
  | { kind: "lecture"; lectureId: string };

function sourceOf(item: DriveItem): Source | null {
  if (item.type === "recording") return { kind: "lecture", lectureId: item.entry.lectureId };
  if (item.type !== "file") return null;
  const { name, size, modified } = item.entry;
  const file = { path: item.key, size, modified };
  const kind = kindOf(name, item.row?.category);
  if (kind === "image") return { kind: "image", path: item.key };
  if (kind === "canvas" || kind === "notebook") return { kind: "drawing", file, notebook: kind === "notebook", fileId: item.row?.id };
  const format = previewFormat(name, kind);
  if (format) return size > 0 && size <= MAX_TEXT_BYTES ? { kind: "text", file, format } : null;
  const pdf = docPdfRelPath({ filename: name, relative_path: item.key });
  return pdf ? { kind: "pdf", path: pdf, file } : null;
}

/** Whether `source`'s picture is already in memory. */
function isKept(source: Source): boolean {
  switch (source.kind) {
    case "pdf": return cachedPdfPicture(source.file) != null;
    case "drawing": return cachedDrawnPicture(source.file, source.notebook, source.fileId) != null;
    case "text": return cachedPreview(source.file) != null;
    case "image": return true;
    case "lecture": return lectureFrames.has(source.lectureId);
  }
}

/** Each recording's frame path from `lecture_thumbnail`, once asked. */
const lectureFrames = new Map<string, string>();

/** Covers the glyph once it has something to show. */
function Picture({ source, onFail }: { source: Source; onFail: () => void }) {
  if (source.kind === "pdf") return <PdfThumb file={source.file} path={source.path} onFail={onFail} />;
  if (source.kind === "drawing") return <CanvasThumb file={source.file} notebook={source.notebook} fileId={source.fileId} onFail={onFail} />;
  if (source.kind === "text") return <TextThumb file={source.file} format={source.format} onFail={onFail} />;
  return <LoadedPicture source={source} onFail={onFail} />;
}

type FileSource = Extract<Source, { kind: "image" | "lecture" }>;

/** The picture's URL: an asset URL for a library image or a recording's
 *  frame; null while loading or when there is none. */
function usePictureUrl(source: FileSource, onFail: () => void): string | null {
  const dataDir = useDataDir();
  const [url, setUrl] = useState<string | null>(() => keptUrl(source, dataDir));
  const fail = useRef(onFail);
  fail.current = onFail;
  const key = source.kind === "lecture" ? source.lectureId : source.path;

  useEffect(() => {
    let live = true;
    const settle = (next: string | null) => {
      if (!live) return;
      if (next) setUrl(next);
      else fail.current();
    };
    if (source.kind === "image") {
      if (dataDir) settle(convertFileSrc(`${dataDir}/${source.path}`));
    } else {
      invoke<string | null>("lecture_thumbnail", { lectureId: source.lectureId })
        .then((path) => {
          if (path) lectureFrames.set(source.lectureId, path);
          settle(path ? convertFileSrc(path) : null);
        })
        .catch(() => settle(null));
    }
    return () => { live = false; };
    // `key` names the source; the object itself is rebuilt every render.
  }, [key, dataDir]);

  return url;
}

/** The URL a library image or a recording seen before has right away. */
function keptUrl(source: FileSource, dataDir: string | null | undefined): string | null {
  if (source.kind === "image") return dataDir ? convertFileSrc(`${dataDir}/${source.path}`) : null;
  const frame = lectureFrames.get(source.lectureId);
  return frame ? convertFileSrc(frame) : null;
}

function LoadedPicture({ source, onFail }: { source: FileSource; onFail: () => void }) {
  const url = usePictureUrl(source, onFail);
  if (!url) return null;
  return <img src={url} alt="" draggable={false} onError={onFail} className="h-full w-full object-cover" />;
}
