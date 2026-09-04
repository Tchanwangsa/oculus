import {
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
  type Icon,
} from "@phosphor-icons/react";

/**
 * Office formats the scraper stores as themselves plus a derived sibling PDF
 * ("deck.pptx" → "deck.pptx.pdf"). Mirrors OFFICE_EXTS in paths.rs.
 */
export const OFFICE_EXTS = ["pptx", "docx", "ppt", "doc"];

function ext(filename: string): string {
  const i = filename.lastIndexOf(".");
  return i === -1 ? "" : filename.slice(i + 1).toLowerCase();
}

export function isOfficeFile(filename: string): boolean {
  return OFFICE_EXTS.includes(ext(filename));
}

/**
 * The PDF that viewing/parsing/embedding operate on: the file itself for real
 * PDFs, the derived sibling for Office documents, null for everything else.
 * Mirrors `doc_pdf_rel` in paths.rs.
 */
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

/**
 * The sidecar's markdown output for a PDF-backed file. Artifacts are keyed on
 * the parsed PDF's stem, which for both plain PDFs ("a.pdf" → "a.md") and
 * Office docs ("deck.pptx" via "deck.pptx.pdf" → "deck.pptx.md") is the
 * library path with any trailing ".pdf" gone.
 */
export function parsedMdRelPath(file: { filename: string; relative_path: string }): string | null {
  const e = ext(file.filename);
  if (e === "pdf") return file.relative_path.replace(/\.pdf$/i, ".md");
  if (OFFICE_EXTS.includes(e)) return `${file.relative_path}.md`;
  return null;
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
    case "xlsx": return FileXls;
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
