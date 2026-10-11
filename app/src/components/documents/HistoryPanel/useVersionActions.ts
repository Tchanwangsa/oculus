import { useEffect, useState, type Dispatch, type RefObject, type SetStateAction } from "react";

import type { DbFile } from "@/lib/db";
import {
  deleteVersion,
  relabelVersion,
  restoreAsCopy,
  snapshot,
  versionTitle,
  type DocumentVersion,
} from "@/lib/notes/documentVersions";
import { filePagePath } from "@/lib/files/openFile";
import { useTabStore } from "@/stores/shell/tabStore";

/** What the selected version can do: come back as a copy or over the open
 *  note, be renamed (checkpoints) or deleted. */
export function useVersionActions({
  file,
  subject,
  selectedId,
  setSelectedId,
  texts,
  textOf,
  currentText,
  replaceText,
}: {
  file: DbFile;
  subject: { id: number; code: string } | null;
  selectedId: number | null;
  setSelectedId: Dispatch<SetStateAction<number | null>>;
  texts: RefObject<Map<number, string>>;
  textOf: (v: DocumentVersion) => Promise<string>;
  currentText: () => string | null;
  replaceText: (text: string) => boolean;
}) {
  const fileId = file.id;
  /** The action running on the selected version, and the last one's failure. */
  const [busy, setBusy] = useState<"copy" | "replace" | "label" | "delete" | null>(null);
  const [actionError, setActionError] = useState<string | null>(null);
  const [confirm, setConfirm] = useState<"replace" | "delete" | null>(null);
  const [renaming, setRenaming] = useState(false);

  // A different row starts clean.
  useEffect(() => {
    setActionError(null);
    setRenaming(false);
  }, [selectedId]);

  const run = async (kind: NonNullable<typeof busy>, action: () => Promise<void>) => {
    setBusy(kind);
    setActionError(null);
    try {
      await action();
    } catch (e) {
      setActionError(String(e));
    } finally {
      setBusy(null);
    }
  };

  const restoreCopy = (v: DocumentVersion) =>
    run("copy", async () => {
      if (!subject) throw new Error("The note's subject hasn't loaded yet.");
      const row = await restoreAsCopy(subject, file, v);
      useTabStore.getState().addTab(filePagePath(row.subject_id, row.relative_path));
    });

  const replace = (v: DocumentVersion) =>
    run("replace", async () => {
      const text = await textOf(v);
      const current = currentText();
      if (current == null) throw new Error("The note isn't open in an editor.");
      if (current === text) return;
      await snapshot(fileId, current, "restore", `Before restoring ${versionTitle(v)}`);
      if (!replaceText(text)) throw new Error("The note isn't open in an editor.");
    });

  const relabel = (v: DocumentVersion, label: string) => {
    setRenaming(false);
    const next = label.trim() || null;
    if (next === (v.label ?? null)) return;
    void run("label", () => relabelVersion(v.id, next));
  };

  const remove = (v: DocumentVersion) =>
    run("delete", async () => {
      await deleteVersion(v.id);
      texts.current.delete(v.id);
      setSelectedId((id) => (id === v.id ? null : id));
    });

  return { busy, actionError, confirm, setConfirm, renaming, setRenaming, restoreCopy, replace, relabel, remove };
}
