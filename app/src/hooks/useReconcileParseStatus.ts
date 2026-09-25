import { useEffect } from "react";
import { setParseStatusByPath, type DbFile } from "@/lib/db";
import { isPdfBacked } from "@/lib/fileTypes";
import { useParseStore } from "@/stores/parseStore";
import { scanParsedFiles } from "@/lib/courseFiles";

/** Reads parse status back from disk, for PDFs parsed in an earlier session
 *  that this session's store knows nothing about. */
export function useReconcileParseStatus(files: DbFile[]) {
  const merge = useParseStore((s) => s.merge);
  useEffect(() => {
    const paths = files.filter((f) => isPdfBacked(f.filename)).map((f) => f.relative_path);
    if (paths.length === 0) return;
    scanParsedFiles(paths)
      .then((entries) => {
        if (entries.length === 0) return;
        merge(Object.fromEntries(entries));
        setParseStatusByPath(entries).catch(() => {});
      })
      .catch(() => {});
  }, [files, merge]);
}
