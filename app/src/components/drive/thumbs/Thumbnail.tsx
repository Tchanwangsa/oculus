import { useEffect, useRef, useState } from "react";
import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { Play } from "@phosphor-icons/react";
import { useDataDir } from "@/hooks/backend/useDataDir";
import { docPdfRelPath } from "@/lib/files/fileTypes";
import { kindOf } from "@/lib/drive/filters";
import type { DriveItem } from "@/lib/drive/items";
import { notebookCover } from "@/lib/drive/listing";
import { cn } from "@/lib/utils";
import { ItemIcon } from "../content/ItemIcon";
import { PdfThumb } from "./PdfThumb";
import { useInView } from "./useInView";

/**
 * A card's picture, drawn by what already draws each kind: a PDF's (or an
 * Office file's converted PDF's) first page through the PDF viewer's
 * renderer, a notebook's first page from the pictures its editor writes for
 * chat agents, a recording's frame from `lecture_thumbnail`, an image itself.
 * Anything else, or a picture that can't be had, is the kind's glyph. Nothing
 * loads until the card is near the screen.
 */
export function Thumbnail({ item, className }: { item: DriveItem; className?: string }) {
  const ref = useRef<HTMLDivElement>(null);
  const seen = useInView(ref);
  const [failed, setFailed] = useState(false);
  const source = failed ? null : sourceOf(item);
  return (
    <div ref={ref} className={cn("relative flex items-center justify-center overflow-hidden bg-surface", className)}>
      <ItemIcon item={item} size={28} className="opacity-70" />
      {seen && source ? (
        <div className="absolute inset-0">
          <Picture source={source} onFail={() => setFailed(true)} />
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
  | { kind: "pdf"; path: string }
  | { kind: "image"; path: string }
  | { kind: "notebook"; fileId: number }
  | { kind: "lecture"; lectureId: string };

function sourceOf(item: DriveItem): Source | null {
  if (item.type === "recording") return { kind: "lecture", lectureId: item.entry.lectureId };
  if (item.type !== "file") return null;
  const kind = kindOf(item.entry.name, item.row?.category);
  if (kind === "image") return { kind: "image", path: item.key };
  if (kind === "notebook") return item.row ? { kind: "notebook", fileId: item.row.id } : null;
  const pdf = docPdfRelPath({ filename: item.entry.name, relative_path: item.key });
  return pdf ? { kind: "pdf", path: pdf } : null;
}

/** Covers the glyph once it has something to show. */
function Picture({ source, onFail }: { source: Source; onFail: () => void }) {
  if (source.kind === "pdf") return <PdfThumb path={source.path} onFail={onFail} />;
  return <LoadedPicture source={source} onFail={onFail} />;
}

/** The picture's URL: an asset URL for a library file, a blob for a
 *  notebook's cover; null while loading or when there is none. */
function usePictureUrl(source: Exclude<Source, { kind: "pdf" }>, onFail: () => void): string | null {
  const dataDir = useDataDir();
  const [url, setUrl] = useState<string | null>(null);
  const fail = useRef(onFail);
  fail.current = onFail;
  const key = source.kind === "notebook" ? source.fileId : source.kind === "lecture" ? source.lectureId : source.path;

  useEffect(() => {
    let live = true;
    let blobUrl: string | null = null;
    const settle = (next: string | null) => {
      if (!live) return;
      if (next) setUrl(next);
      else fail.current();
    };
    if (source.kind === "image") {
      if (dataDir) settle(convertFileSrc(`${dataDir}/${source.path}`));
    } else if (source.kind === "notebook") {
      notebookCover(source.fileId)
        .then((blob) => {
          blobUrl = blob ? URL.createObjectURL(blob) : null;
          settle(blobUrl);
        })
        .catch(() => settle(null));
    } else {
      invoke<string | null>("lecture_thumbnail", { lectureId: source.lectureId })
        .then((path) => settle(path ? convertFileSrc(path) : null))
        .catch(() => settle(null));
    }
    return () => {
      live = false;
      if (blobUrl) URL.revokeObjectURL(blobUrl);
    };
    // `key` names the source; the object itself is rebuilt every render.
  }, [key, dataDir]);

  return url;
}

function LoadedPicture({ source, onFail }: {
  source: Exclude<Source, { kind: "pdf" }>;
  onFail: () => void;
}) {
  const url = usePictureUrl(source, onFail);
  if (!url) return null;
  return (
    <img
      src={url}
      alt=""
      draggable={false}
      onError={onFail}
      className={cn("h-full w-full", source.kind === "notebook" ? "bg-white object-contain" : "object-cover")}
    />
  );
}
