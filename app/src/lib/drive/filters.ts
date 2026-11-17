import { isOfficeFile, isSheetFile, isVideoFile, CODE_EXTS } from "@/lib/files/fileTypes";
import type { DbFile } from "@/lib/db";
import type { DriveZone, FileEntry, FolderEntry, FolderListing } from "./listing";

/**
 * The toolbar's Type, From and Modified filters and the folder's search. Any
 * of them switches the browser from one folder to a flat list of everything
 * below it (`listFolder(rel, true)`), narrowed here.
 */

export type DriveKind =
  | "pdf" | "document" | "slides" | "sheet" | "note" | "canvas" | "notebook"
  | "video" | "audio" | "image" | "code" | "archive" | "other";

export const KIND_LABEL: Record<DriveKind, string> = {
  pdf: "PDF",
  document: "Document",
  slides: "Slides",
  sheet: "Spreadsheet",
  note: "Note",
  canvas: "Canvas",
  notebook: "Notebook",
  video: "Video",
  audio: "Audio",
  image: "Image",
  code: "Code",
  archive: "Archive",
  other: "Other",
};

/** In the Type filter's order. */
export const KINDS = Object.keys(KIND_LABEL) as DriveKind[];

const ext = (name: string) => name.slice(name.lastIndexOf(".") + 1).toLowerCase();

/** What a file is, from its name and, for markdown, its row's category: a
 *  synced page is a document, the student's markdown is a note. */
export function kindOf(name: string, category?: string | null): DriveKind {
  const e = name.includes(".") ? ext(name) : "";
  if (e === "pdf") return "pdf";
  if (e === "canvas") return "canvas";
  if (e === "notebook") return "notebook";
  if (e === "ppt" || e === "pptx" || e === "key") return "slides";
  if (isSheetFile(name)) return "sheet";
  if (isOfficeFile(name) || e === "rtf" || e === "pages") return "document";
  if (e === "md") return category == null || category === "document" ? "note" : "document";
  if (e === "txt") return "document";
  if (isVideoFile(name)) return "video";
  if (["mp3", "wav", "m4a", "aac", "flac", "ogg", "opus"].includes(e)) return "audio";
  if (["png", "jpg", "jpeg", "gif", "webp", "svg", "bmp", "heic", "tiff"].includes(e)) return "image";
  if (["zip", "tar", "gz", "7z", "rar", "tgz"].includes(e)) return "archive";
  if (CODE_EXTS.includes(e) || ["ipynb", "json", "html", "css", "sh", "sql"].includes(e)) return "code";
  return "other";
}

/** Where a file came from: the student's own, an Ed thread or lesson, or the
 *  rest of what the sync brings (Canvas). */
export type DriveSource = "canvas" | "ed" | "yours";

export const SOURCE_LABEL: Record<DriveSource, string> = {
  canvas: "Canvas",
  ed: "Ed",
  yours: "Your files",
};

export const SOURCES = Object.keys(SOURCE_LABEL) as DriveSource[];

export function sourceOf(zone: DriveZone, category: string | null | undefined): DriveSource {
  if (zone === "own") return "yours";
  return category === "ed" || category === "lessons" ? "ed" : "canvas";
}

export type ModifiedRange = "any" | "today" | "week" | "month" | "year";

export const MODIFIED_LABEL: Record<ModifiedRange, string> = {
  any: "Any time",
  today: "Today",
  week: "Last 7 days",
  month: "Last 30 days",
  year: "This year",
};

export const MODIFIED_RANGES = Object.keys(MODIFIED_LABEL) as ModifiedRange[];

/** The earliest modification time a range keeps, or null for any. */
export function modifiedSince(range: ModifiedRange, now: Date): number | null {
  const day = new Date(now.getFullYear(), now.getMonth(), now.getDate());
  switch (range) {
    case "any": return null;
    case "today": return day.getTime();
    case "week": return day.getTime() - 6 * 86_400_000;
    case "month": return day.getTime() - 29 * 86_400_000;
    case "year": return new Date(now.getFullYear(), 0, 1).getTime();
  }
}

export interface DriveFilters {
  query: string;
  kinds: ReadonlySet<DriveKind>;
  sources: ReadonlySet<DriveSource>;
  modified: ModifiedRange;
}

export const NO_FILTERS: DriveFilters = { query: "", kinds: new Set(), sources: new Set(), modified: "any" };

/** Filters other than the search: these leave folders out of the flat list. */
export function narrowsFiles(f: DriveFilters): boolean {
  return f.kinds.size > 0 || f.sources.size > 0 || f.modified !== "any";
}

/** True when the browser shows the flat list below the folder. */
export function isFlat(f: DriveFilters): boolean {
  return f.query.trim() !== "" || narrowsFiles(f);
}

/** Every whitespace-separated word of the query is in the name, any case. */
export function matchesQuery(name: string, query: string): boolean {
  const words = query.toLowerCase().split(/\s+/).filter(Boolean);
  const hay = name.toLowerCase();
  return words.every((w) => hay.includes(w));
}

/** The flat list's entries: folders only while just searching, files that
 *  pass every filter. `rows` joins each file to its `files` row. */
export function filterListing(
  listing: Pick<FolderListing, "folders" | "files">,
  rows: ReadonlyMap<string, DbFile>,
  f: DriveFilters,
  now: Date,
): { folders: FolderEntry[]; files: FileEntry[] } {
  const since = modifiedSince(f.modified, now);
  const folders = narrowsFiles(f) ? [] : listing.folders.filter((d) => matchesQuery(d.name, f.query));
  const files = listing.files.filter((file) => {
    const row = rows.get(file.relativePath);
    if (!matchesQuery(file.name, f.query)) return false;
    if (f.kinds.size && !f.kinds.has(kindOf(file.name, row?.category))) return false;
    if (f.sources.size && !f.sources.has(sourceOf(file.zone, row?.category))) return false;
    if (since != null && (file.modified ?? 0) < since) return false;
    return true;
  });
  return { folders, files };
}
