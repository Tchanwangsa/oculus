import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { parseModuleToc, type ModuleToc } from "@/lib/moduleToc";
import type { DbFile } from "@/lib/db";

export interface LoadedModule extends ModuleToc {
  /** `modules/03-week-1.md` — hrefs inside resolve against this. */
  relPath: string;
}

/**
 * Loads and parses every module TOC for a subject. `null` while loading.
 * Interim: goes away once modules/module_items get their own tables.
 */
export function useModuleTocs(
  moduleFiles: DbFile[],
  filesLoading: boolean,
): LoadedModule[] | null {
  const [modules, setModules] = useState<LoadedModule[] | null>(null);

  useEffect(() => {
    if (moduleFiles.length === 0) {
      if (!filesLoading) setModules([]);
      return;
    }
    let cancelled = false;
    Promise.all(
      moduleFiles.map(async (f) => {
        const md = await invoke<string>("read_course_file", {
          relativePath: f.relative_path,
        }).catch(() => "");
        return { ...parseModuleToc(md), relPath: f.relative_path };
      }),
    ).then((loaded) => {
      if (cancelled) return;
      // Filenames are `NN-slug.md`, NN being the Canvas module position.
      setModules(loaded.sort((a, b) => a.relPath.localeCompare(b.relPath)));
    });
    return () => {
      cancelled = true;
    };
  }, [moduleFiles, filesLoading]);

  return modules;
}
