import { useMemo } from "react";
import { parseModuleToc, type ModuleToc } from "@/lib/pipeline/moduleToc";
import type { DbFile } from "@/lib/db";
import { useCourseFileData } from "@/hooks/data/useCourseFileData";

export interface LoadedModule extends ModuleToc {
  /** `modules/03-week-1.md` — hrefs inside resolve against this. */
  relPath: string;
}

const parse = (markdown: string, file: DbFile): LoadedModule =>
  ({ ...parseModuleToc(markdown), relPath: file.relative_path });

/** Loads and parses every module TOC for a subject. `null` while loading. */
export function useModuleTocs(
  moduleFiles: DbFile[],
  filesLoading: boolean,
): LoadedModule[] | null {
  const modules = useCourseFileData(moduleFiles, filesLoading, parse);
  // Filenames are `NN-slug.md`, NN being the Canvas module position.
  return useMemo(
    () => modules && [...modules].sort((a, b) => a.relPath.localeCompare(b.relPath)),
    [modules],
  );
}
