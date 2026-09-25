import { useCallback, useMemo, useState } from "react";
import { addUploads, pickUploads } from "@/lib/uploads";

/** The import half of the Uploads sub-tab, as its pages see it. */
export interface UploadImport {
  /** Names currently being copied and converted, in the order they were
   *  handed over. LibreOffice takes a couple of seconds per document, and a
   *  picker that closes onto an unchanged list reads as a click that did
   *  nothing — so the names are shown as rows until the real ones replace
   *  them. */
  importing: string[];
  /** Per-file problems from the last add, kept until the next one. */
  problems: string[];
  /** Copy these paths into the subject and start the pipeline on each. */
  add: (paths: string[]) => Promise<void>;
  /** The native open panel, then `add` on whatever it returns. */
  choose: () => Promise<void>;
  /** Put one problem where the add's would go — the delete dialog's failure
   *  has no other row to land on. */
  report: (problem: string) => void;
  /** True while an add is in flight. */
  busy: boolean;
}

/**
 * State for adding a student's own files to a subject, held **above** the
 * Uploads sub-page — in the Files tab — rather than in it.
 *
 * It lives one level up because the drop target is the whole Files tab
 * (`app/src/pages/subject/FilesPage.tsx`): a folder dragged in from Finder
 * lands whichever sub-tab is showing, and the "Adding…" rows it produces have
 * to be there when the tab switches to Uploads to show them. State inside the
 * Uploads page would be unmounted, and lost, on the sub-tab it was not on.
 *
 * Nothing here knows about the drag or the overlay; those are `useFileDrop`'s
 * and the page's. This is only the copy, the rows while it runs, and what went
 * wrong.
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

  // One object per change rather than per render: the Files tab spreads it
  // into its outlet context, and a fresh identity there re-renders every
  // sub-page on every keystroke anywhere in the tab.
  return useMemo(
    () => ({ importing, problems, add, choose, report, busy: importing.length > 0 }),
    [importing, problems, add, choose, report],
  );
}
