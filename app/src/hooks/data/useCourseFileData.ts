import { useEffect, useMemo, useState } from "react";
import type { DbFile } from "@/lib/db";
import { createCourseFileDataLoader } from "@/lib/files/courseFiles";

/** Load scraped header metadata without allowing an old subject's reads to
 *  overwrite the current one. Keep `parse` stable (a module-level function). */
export function useCourseFileData<T>(
  files: DbFile[],
  filesLoading: boolean,
  parse: (markdown: string, file: DbFile) => T,
): T[] | null {
  const [data, setData] = useState<T[] | null>(null);
  const load = useMemo(() => createCourseFileDataLoader(parse), [parse]);
  useEffect(() => {
    // A refresh retains the previous rows while SQLite is reading them. Wait
    // for the current list instead of re-reading those old files as well.
    if (filesLoading) return;
    let cancelled = false;
    load(files).then((loaded) => {
      if (!cancelled) setData(loaded);
    });
    return () => { cancelled = true; };
  }, [files, filesLoading, load]);
  return data;
}
