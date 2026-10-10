import { useCallback, useMemo, useState } from "react";
import { addUploads, pickUploads } from "@/lib/files/uploads";

export interface UploadImport {
  /** Names being copied and converted, shown as rows until the real ones land
   *  so a slow Office conversion does not read as a click that did nothing. */
  importing: string[];
  /** From the last add, kept until the next one. */
  problems: string[];
  add: (paths: string[]) => Promise<void>;
  choose: () => Promise<void>;
  /** Show one problem where the add's would go (the delete dialog's failure). */
  report: (problem: string) => void;
  busy: boolean;
}

/**
 * Upload state, held in the Files tab (`FilesPage.tsx`) rather than the Uploads
 * sub-page: the whole tab is the drop target, and state in the sub-page would
 * be lost while another sub-tab shows. The drag itself is `useFileDrop`'s.
 */
export function useUploadImport(subject: { id: number; code: string }): UploadImport {
  const [importing, setImporting] = useState<string[]>([]);
  const [problems, setProblems] = useState<string[]>([]);

  const add = useCallback(
    async (paths: string[]) => {
      if (paths.length === 0) return;
      setProblems([]);
      setImporting(paths.map((p) => p.split("/").pop() ?? p));
      try {
        const outcomes = await addUploads(subject, paths);
        setProblems(
          outcomes
            .filter((o) => o.error)
            .map((o) => `${o.source}: ${o.error}`),
        );
      } catch (e) {
        setProblems([String(e)]);
      } finally {
        setImporting([]);
      }
    },
    [subject],
  );

  const choose = useCallback(async () => {
    try {
      await add(await pickUploads());
    } catch (e) {
      setProblems([String(e)]);
    }
  }, [add]);

  const report = useCallback((problem: string) => setProblems([problem]), []);

  // Stable identity: the Files tab puts this in its outlet context.
  return useMemo(
    () => ({ importing, problems, add, choose, report, busy: importing.length > 0 }),
    [importing, problems, add, choose, report],
  );
}
