import { useEffect, useReducer, useRef, useState } from "react";

import type { DocumentVersion } from "@/lib/notes/documentVersions";

import type { CompareWith } from "./useCompare";

/** What the page shows: a version's text and the base its changes are marked
 *  against (null: no marks). */
export interface ShownVersion {
  id: number;
  text: string;
  base: string | null;
}

/**
 * The selected version's text and its base, landed together so the marks
 * never pair one version with another's base; the last pair stays on the
 * page while the next loads. Previous is the next-older row, so the oldest
 * compares with the note now.
 */
export function useShownVersion({
  versions,
  selected,
  compare,
  showChanges,
  textOf,
  currentText,
}: {
  versions: DocumentVersion[] | null;
  selected: DocumentVersion | null;
  compare: CompareWith;
  showChanges: boolean;
  textOf: (v: DocumentVersion) => Promise<string>;
  currentText: () => string | null;
}) {
  const [shown, setShown] = useState<ShownVersion | null>(null);
  const [shownError, setShownError] = useState<string | null>(null);
  // Re-reads the note's text for a Current note base, after a replace.
  const [currentSeq, rereadCurrent] = useReducer((n: number) => n + 1, 0);
  const io = useRef({ textOf, currentText });
  io.current = { textOf, currentText };

  const index = selected && versions ? versions.indexOf(selected) : -1;
  const older = index >= 0 ? (versions?.[index + 1] ?? null) : null;
  const effective: CompareWith = compare === "previous" && !older ? "current" : compare;

  const selectedId = selected?.id;
  useEffect(() => {
    // No row (the last one deleted): nothing stays on the page.
    if (!selected) {
      setShown(null);
      return;
    }
    let live = true;
    setShownError(null);
    const { textOf, currentText } = io.current;
    const base =
      !showChanges ? Promise.resolve(null)
      : effective === "current" || !older ? Promise.resolve(currentText())
      : textOf(older);
    Promise.all([textOf(selected), base])
      .then(([text, base]) => live && setShown({ id: selected.id, text, base }))
      .catch((e) => live && setShownError(String(e)));
    return () => {
      live = false;
    };
    // Ids, not rows: a reread list hands out new objects for the same versions.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [selectedId, older?.id, effective, showChanges, currentSeq]);

  return { shown, shownError, older, effective, rereadCurrent };
}
