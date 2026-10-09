import {
  ChatsCircle,
  File,
  FileArchive,
  FileAudio,
  FileCode,
  FileCsv,
  FileDoc,
  FileImage,
  FileJpg,
  FileMd,
  FilePdf,
  FilePng,
  FilePpt,
  FileSvg,
  FileText,
  FileVideo,
  FileXls,
  FileZip,
  Megaphone,
  NotePencil,
  PencilLine,
  Rocket,
  Stack,
  type Icon,
} from "@phosphor-icons/react";

/** Office formats stored with a derived sibling PDF ("deck.pptx" →
 *  "deck.pptx.pdf"). Mirrors OFFICE_EXTS in paths.rs. */
export const OFFICE_EXTS = ["pptx", "docx", "ppt", "doc"];

/** Spreadsheets, converted in Rust to text beside them ("marks.xlsx" →
 *  "marks.xlsx.md", one page per sheet); never PDF-backed, never embedded.
 *  Mirrors SHEET_EXTS in paths.rs. */
export const SHEET_EXTS = ["xlsx", "xlsm", "xls", "ods", "csv"];

const sqlList = (exts: string[]) => `(${exts.map((e) => `'${e}'`).join(", ")})`;

/** PDF plus OFFICE_EXTS as a SQL list, for `lower(file_type) IN …`. */
export const PDF_BACKED_SQL_LIST = sqlList(["pdf", ...OFFICE_EXTS]);

/** Every file with a File Activity row: the PDF-backed ones and spreadsheets. */
export const PIPELINE_SQL_LIST = sqlList(["pdf", ...OFFICE_EXTS, ...SHEET_EXTS]);

function ext(filename: string): string {
  const i = filename.lastIndexOf(".");
  return i === -1 ? "" : filename.slice(i + 1).toLowerCase();
}

export function isOfficeFile(filename: string): boolean {
  return OFFICE_EXTS.includes(ext(filename));
}

/** The PDF that viewing/parsing/embedding use: the file itself, the derived
 *  sibling for Office documents, else null. Mirrors `doc_pdf_rel` in paths.rs. */
export function docPdfRelPath(file: { filename: string; relative_path: string }): string | null {
  const e = ext(file.filename);
  if (e === "pdf") return file.relative_path;
  if (OFFICE_EXTS.includes(e)) return `${file.relative_path}.pdf`;
  return null;
}

/** True when the file goes through the PDF parse/embed pipeline. */
export function isPdfBacked(filename: string): boolean {
  return ext(filename) === "pdf" || isOfficeFile(filename);
}

export function isSheetFile(filename: string): boolean {
  return SHEET_EXTS.includes(ext(filename));
}

/** True when the file has a File Activity row: download → parse (for a
 *  spreadsheet, its conversion to text), and embed only if PDF-backed. */
export function isPipelineFile(filename: string): boolean {
  return isPdfBacked(filename) || isSheetFile(filename);
}

/** Video formats a library file can hold — module videos land as
 *  `files/<name>.mp4`. Mirrors VIDEO_EXTS in sync.rs. */
const VIDEO_EXTS = ["mp4", "mov", "m4v", "webm"];

/** True for a video file. Videos never enter the parse/embed pipeline. */
export function isVideoFile(filename: string): boolean {
  return VIDEO_EXTS.includes(ext(filename));
}

/** The derived markdown of a PDF-backed file or spreadsheet: "a.pdf" →
 *  "a.md", "deck.pptx" → "deck.pptx.md", "marks.xlsx" → "marks.xlsx.md". */
export function parsedMdRelPath(file: { filename: string; relative_path: string }): string | null {
  const e = ext(file.filename);
  if (e === "pdf") return file.relative_path.replace(/\.pdf$/i, ".md");
  if (OFFICE_EXTS.includes(e) || SHEET_EXTS.includes(e)) return `${file.relative_path}.md`;
  return null;
}

/** A file's icon in a mixed-category list, matching its subject tab's glyph;
 *  only downloads fall through to `fileIconFor`. */
export function categoryIconFor(file: {
  category: string | null;
  filename: string;
}): Icon {
  switch (file.category) {
    case "announcement": return Megaphone;
    case "assignment": return PencilLine;
    case "quiz": return Rocket;
    case "ed": return ChatsCircle;
    case "module": return Stack;
    case "file":
    case "image": return fileIconFor(file.filename);
    case "document": return NotePencil;
    default: return FileText;
  }
}

/** Phosphor's per-format file icon, `File` when the extension has none. */
export function fileIconFor(filename: string): Icon {
  switch (ext(filename)) {
    case "pdf": return FilePdf;
    case "doc":
    case "docx": return FileDoc;
    case "ppt":
    case "pptx": return FilePpt;
    case "xls":
    case "xlsx":
    case "xlsm":
    case "ods": return FileXls;
    case "csv": return FileCsv;
    case "md": return FileMd;
    case "txt": return FileText;
    case "png": return FilePng;
    case "jpg":
    case "jpeg": return FileJpg;
    case "svg": return FileSvg;
    case "gif":
    case "webp":
    case "bmp": return FileImage;
    case "zip": return FileZip;
    case "tar":
    case "gz":
    case "7z":
    case "rar": return FileArchive;
    case "mp4":
    case "mov":
    case "m4v":
    case "mkv":
    case "webm": return FileVideo;
    case "mp3":
    case "wav":
    case "m4a": return FileAudio;
    case "py":
    case "js":
    case "ts":
    case "java":
    case "c":
    case "cpp":
    case "rs":
    case "ipynb":
    case "json": return FileCode;
    default: return File;
  }
}

/**
 * The inverse of `parsedMdRelPath`, for an agent citing derived markdown that
 * has no row of its own: "a.md" → "a.pdf", "deck.pptx.md" → "deck.pptx",
 * "marks.xlsx.md" → "marks.xlsx". Only called after a direct lookup misses.
 */
export function parsedMdSource(path: string): string | null {
  if (!/\.md$/i.test(path)) return null;
  const stem = path.slice(0, -3);
  return isOfficeFile(stem) || isSheetFile(stem) ? stem : `${stem}.pdf`;
}
