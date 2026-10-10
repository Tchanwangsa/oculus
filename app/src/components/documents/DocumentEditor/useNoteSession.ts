import { useCallback, useEffect, useRef, useState, type RefObject } from "react";
import { isolateHistory } from "@codemirror/commands";
import type { EditorView } from "@codemirror/view";

import { documentSessions, type DocumentLease } from "@/lib/notes/documentSessions";
import { saveCheckpoint } from "@/lib/notes/documentVersions";
import type { DbFile } from "@/lib/db";

import type { DocumentActions, SaveStatus } from "../DocumentControls";
import { countWords } from "./countWords";

/**
 * The note's place in its shared session (`@/lib/notes/documentSessions`):
 * the lease, whether its text has loaded, the last save and word count, and
 * what the editor does with them (flush, the header's Save version, a
 * restored version replacing the text).
 */
export function useNoteSession(
  file: DbFile,
  onStatus: (status: SaveStatus) => void,
  actions: RefObject<DocumentActions | null> | undefined,
  viewRef: RefObject<EditorView | null>,
) {
  /** The row whose text the session has; the view is made once it does. */
  const [loadedId, setLoadedId] = useState<number | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  /** The session's last save, and the words on disk — both move on save,
   *  not per keystroke. The row's `modified_at` isn't re-read on save. */
  const [savedAt, setSavedAt] = useState<number | null>(null);
  const [words, setWords] = useState(0);

  const leaseRef = useRef<DocumentLease<DbFile> | null>(null);
  // Updated after commit, not during render, so the cleanup on a file switch
  // (which React runs before new effects) still sees the old file.
  const fileRef = useRef(file);
  useEffect(() => {
    fileRef.current = file;
  });
  const statusRef = useRef(onStatus);
  statusRef.current = onStatus;

  /** Write the note if it differs from disk. Idempotent. */
  const flush = useCallback(
    (): Promise<void> => leaseRef.current?.flush() ?? Promise.resolve(),
    [],
  );

  /** What the editor shows now: the view's text, else the session's. */
  const currentText = useCallback(
    (): string | null => viewRef.current?.state.doc.toString() ?? leaseRef.current?.text ?? null,
    [viewRef],
  );

  // Writes what is pending first, so the version and the file agree.
  useEffect(() => {
    if (!actions) return;
    const mine: DocumentActions = {
      saveVersion: async (label) => {
        await flush();
        const text = currentText();
        if (text == null) throw new Error("The note is still loading.");
        return saveCheckpoint(fileRef.current.id, text, label);
      },
    };
    actions.current = mine;
    return () => {
      if (actions.current === mine) actions.current = null;
    };
  }, [actions, flush, currentText]);

  /** A restored version replaces the whole text as one ordinary edit, kept
   *  out of neighbouring typing's undo step: it saves through the session,
   *  reaches the note's other views, and ⌘Z takes it back. */
  const replaceText = useCallback((text: string): boolean => {
    const v = viewRef.current;
    if (!v) return false;
    v.dispatch({
      changes: { from: 0, to: v.state.doc.length, insert: text },
      annotations: isolateHistory.of("full"),
    });
    return true;
  }, [viewRef]);

  // Join the note's session, keyed on the row, not its path: a rename must
  // not reload the text. Leaving writes anything unsaved (the session does).
  useEffect(() => {
    let live = true;
    setLoadError(null);
    const lease = documentSessions.open(file, {
      status: (s) => statusRef.current(s),
      saved: (text, at) => {
        setSavedAt(at);
        setWords(countWords(text));
      },
    });
    leaseRef.current = lease;
    const ready = () => {
      setWords(countWords(lease.savedText));
      setSavedAt(lease.savedAt);
      setLoadedId(file.id);
    };
    if (lease.text !== null) ready();
    else {
      lease
        .load()
        .then(() => live && ready())
        .catch((e) => live && setLoadError(String(e)));
    }
    return () => {
      live = false;
      if (leaseRef.current === lease) leaseRef.current = null;
      lease.release();
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [file.id]);

  return {
    fileRef,
    statusRef,
    leaseRef,
    loadedId,
    loadError,
    savedAt,
    words,
    flush,
    currentText,
    replaceText,
  };
}
