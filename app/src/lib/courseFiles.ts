import { invoke } from "@tauri-apps/api/core";
import { shallow } from "zustand/shallow";
import type { DbFile } from "@/lib/db";

/** A text file under the library, by its library-relative path. */
export function readCourseFile(relativePath: string): Promise<string> {
  return invoke<string>("read_course_file", { relativePath });
}

/** A cheap existence probe: parsed markdown bytes stay on disk until shown. */
export function courseFileHasContent(relativePath: string): Promise<boolean> {
  return invoke<boolean>("course_file_has_content", { relativePath });
}

/** Queues a PDF-backed file for parsing; progress arrives as `parse-status`. */
export function parseFile(subjectId: number, subjectCode: string, relativePath: string) {
  return invoke("parse_file", { subjectId, subjectCode, relativePath });
}

/** `[relativePath, parse mode]` for each file whose markdown is on disk. */
export function scanParsedFiles(relativePaths: string[]): Promise<[string, string][]> {
  return invoke<[string, string][]>("scan_parsed_files", { relativePaths });
}

/** Metadata readers keep only parsed values for their current file list. Row
 *  snapshots include scrape and access stamps, so a changed file is re-read
 *  and a parser that carries its DbFile always receives the current row. */
export function createCourseFileDataLoader<T>(
  parse: (markdown: string, file: DbFile) => T,
  read: (relativePath: string) => Promise<string> = readCourseFile,
): (files: DbFile[]) => Promise<T[]> {
  type Entry = { file: DbFile; value: Promise<T> };
  let entries = new Map<string, Entry>();
  let values: Promise<T>[] = [];
  let loaded: Promise<T[]> | null = null;
  return (files) => {
    const previous = entries;
    entries = new Map();
    const next = files.map((file) => {
      const path = file.relative_path;
      const cached = previous.get(path);
      if (cached && shallow(cached.file, file)) {
        entries.set(path, cached);
        return cached.value;
      }
      const entry: Entry = {
        file,
        value: read(path).catch(() => {
          // A missing file is still displayed by its fallback metadata, but a
          // later refresh retries instead of caching the failed read.
          if (entries.get(path) === entry) entries.delete(path);
          return "";
        }).then((markdown) => parse(markdown, file)),
      };
      entries.set(path, entry);
      return entry.value;
    });
    if (loaded && shallow(values, next)) return loaded;
    values = next;
    return loaded = Promise.all(next);
  };
}
