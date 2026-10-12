import { useMemo } from "react";
import { parseModuleToc, type ModuleToc } from "@/lib/pipeline/moduleToc";
import type { DbFile } from "@/lib/db";
import { useCourseFileData } from "@/hooks/data/useCourseFileData";

export interface LoadedModule extends ModuleToc {
  /** `modules/03-week-1.md` — hrefs inside resolve against this. */
  relPath: string;
  scrapedAt: string;
}

const parse = (markdown: string, file: DbFile): LoadedModule =>
  ({ ...parseModuleToc(markdown), relPath: file.relative_path, scrapedAt: file.scraped_at });

/** Loads and parses every module TOC for a subject. `null` while loading. */
export function useModuleTocs(
  moduleFiles: DbFile[],
  filesLoading: boolean,
): LoadedModule[] | null {
  const modules = useCourseFileData(moduleFiles, filesLoading, parse);
  // Filenames are `NN-slug.md`, NN being the Canvas module position. When
  // Canvas reorders a module, its old position path stays in the library; use
  // the newest TOC for that position, then collapse exact duplicate entries.
  return useMemo(
    () => {
      if (!modules) return null;
      const latestByPosition = new Map<string, LoadedModule>();
      for (const module of modules) {
        const position = /(?:^|\/)modules\/(\d+)-/.exec(module.relPath)?.[1];
        const key = position ?? module.relPath;
        const current = latestByPosition.get(key);
        if (!current || module.scrapedAt > current.scrapedAt) {
          latestByPosition.set(key, module);
        }
      }
      const seen = new Set<string>();
      return [...latestByPosition.values()]
        .sort((a, b) => a.relPath.localeCompare(b.relPath))
        .filter((module) => {
          const identity = JSON.stringify([module.title, module.sections]);
          if (seen.has(identity)) return false;
          seen.add(identity);
          return true;
        });
    },
    [modules],
  );
}
