import { invoke } from "@tauri-apps/api/core";

/** A text file under the library, by its library-relative path. */
export function readCourseFile(relativePath: string): Promise<string> {
  return invoke<string>("read_course_file", { relativePath });
}

/** Queues a PDF-backed file for parsing; progress arrives as `parse-status`. */
export function parseFile(subjectId: number, subjectCode: string, relativePath: string) {
  return invoke("parse_file", { subjectId, subjectCode, relativePath });
}

/** `[relativePath, parse mode]` for each file whose markdown is on disk. */
export function scanParsedFiles(relativePaths: string[]): Promise<[string, string][]> {
  return invoke<[string, string][]>("scan_parsed_files", { relativePaths });
}
