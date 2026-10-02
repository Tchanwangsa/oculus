import { useEffect, useState } from "react";
import type { DbFile } from "@/lib/db";
import { readCourseFile } from "@/lib/courseFiles";

/** Load scraped header metadata without allowing an old subject's reads to
 *  overwrite the current one. Keep `parse` stable (a module-level function). */
export function useCourseFileData<T>(
  files: DbFile[],
  filesLoading: boolean,
  parse: (markdown: string, file: DbFile) => T,
): T[] | null {
  const [data, setData] = useState<T[] | null>(null);
  useEffect(() => {
    if (files.length === 0) {
      if (!filesLoading) setData([]);
      return;
    }
    let cancelled = false;
    Promise.all(files.map(async (file) =>
      parse(await readCourseFile(file.relative_path).catch(() => ""), file),
    )).then((loaded) => {
      if (!cancelled) setData(loaded);
    });
    return () => { cancelled = true; };
  }, [files, filesLoading, parse]);
  return data;
}
