import { useCallback, useEffect, useRef, useState, type KeyboardEvent as ReactKeyboardEvent, type RefObject } from "react";
import type { EditorView } from "@codemirror/view";

import { renameDocument } from "@/lib/notes/documents";
import type { DocumentLease } from "@/lib/notes/documentSessions";
import { fileTitle } from "@/lib/files/openFile";
import type { DbFile } from "@/lib/db";

import type { SaveStatus } from "../DocumentControls";

/** A freshly created note, still wearing the name Rust gave it. */
const UNTITLED = /^Untitled(?:-\d+)?$/;

/** The title field: it follows a rename, takes focus on a new note, and a
 *  committed edit renames the file through the session. */
export function useNoteTitle({
  file,
  fileRef,
  leaseRef,
  statusRef,
  viewRef,
  loaded,
}: {
  file: DbFile;
  fileRef: RefObject<DbFile>;
  leaseRef: RefObject<DocumentLease<DbFile> | null>;
  statusRef: RefObject<(status: SaveStatus) => void>;
  viewRef: RefObject<EditorView | null>;
  loaded: boolean;
}) {
  const [title, setTitle] = useState(() => fileTitle(file));
  const titleRef = useRef<HTMLInputElement>(null);

  // Follow a rename unless the title field is being edited.
  useEffect(() => {
    if (document.activeElement !== titleRef.current) setTitle(fileTitle(file));
  }, [file.filename, file.category]); // eslint-disable-line react-hooks/exhaustive-deps

  // A new note opens on its title, selected, so typing replaces "Untitled".
  useEffect(() => {
    if (!loaded || !UNTITLED.test(fileTitle(fileRef.current))) return;
    titleRef.current?.focus();
    titleRef.current?.select();
  }, [loaded, fileRef]);

  const commitTitle = useCallback(async () => {
    const next = title.trim();
    const current = fileTitle(fileRef.current);
    if (!next || next === current) {
      setTitle(current);
      return;
    }
    const lease = leaseRef.current;
    if (!lease) {
      setTitle(current);
      return;
    }
    try {
      // A failed save keeps its error status and must not move the unsaved note.
      const moved = await lease.rename(async (note) => {
        const path = await renameDocument(note, next);
        return { ...note, relative_path: path, filename: path.slice(path.lastIndexOf("/") + 1) };
      });
      if (!moved) {
        setTitle(current);
        return;
      }
      // Until the row reloads, suggestions and `@` search use the new path.
      fileRef.current = {
        ...fileRef.current,
        relative_path: lease.file.relative_path,
        filename: lease.file.filename,
      };
      setTitle(fileTitle(fileRef.current));
    } catch (e) {
      statusRef.current({ state: "error", message: String(e) });
      setTitle(current);
    }
  }, [title, fileRef, leaseRef, statusRef]);

  const onTitleKeyDown = (e: ReactKeyboardEvent<HTMLInputElement>) => {
    if (e.key === "Enter") {
      e.preventDefault();
      // Moving focus blurs the field, and the blur is what commits.
      viewRef.current?.focus();
    } else if (e.key === "Escape") {
      setTitle(fileTitle(fileRef.current));
      e.currentTarget.blur();
    }
  };

  return { title, setTitle, titleRef, commitTitle, onTitleKeyDown };
}
