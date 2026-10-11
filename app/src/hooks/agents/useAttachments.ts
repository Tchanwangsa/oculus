import { useEffect, useRef, useState } from "react";

import { useFileDrop } from "@/hooks/gestures/useFileDrop";
import {
  imagePaths,
  isLongPaste,
  pastedText,
  pendingFromFile,
  pendingFromPath,
  releaseAttachment,
  splitPastedText,
  writeAttachment,
  withAttachments,
  withPastedText,
  type PastedText,
  type PendingAttachment,
} from "@/lib/harness/attachments";
import { useDraftStore } from "@/stores/chat/draftStore";

/** Points at the `@` menu; a box without one passes its own wording. */
const NOT_A_PICTURE = "Only images can be attached — use @ for a course file.";

/** A stable empty list, so the store selector doesn't re-render forever. */
const NO_PASTES: PastedText[] = [];

export interface Attachments {
  items: PendingAttachment[];
  error: string | null;
  writing: boolean;
  dropping: boolean;
  /** From a paste. */
  attach: (files: File[]) => void;
  detach: (id: string) => void;
  /** Long pastes held as cards, kept in `draftStore` under the draft key. */
  texts: PastedText[];
  /** Takes a long plain-text paste as a card; false leaves it to the box. */
  attachText: (text: string) => boolean;
  editText: (id: string, text: string) => void;
  detachText: (id: string) => void;
  /** Lifts the pasted blocks out of handed-back words (a stop, a rewind) into
   *  cards ahead of any held, returning the rest for the box. */
  takeBack: (message: string) => string;
  /** Prepare a trimmed message and its attachments; null keeps the editor intact. */
  prepare: (text: string) => Promise<string | null>;
}

/** Pictures on their way into a message, for every composer: pasted or
 *  dropped (`useFileDrop`), written to disk only on send. `TaskPage` writes on
 *  arrival instead, having no send to defer to. Long pastes ride along as
 *  text cards under `draftKey`, going into the message as `<pasted_text>`
 *  blocks on send. */
export function useAttachments(
  ref: React.RefObject<HTMLElement | null>,
  draftKey: string,
  opts?: {
    notAPicture?: string;
    /** Refuse drops; paste is the caller's own to withhold. */
    disabled?: boolean;
  },
): Attachments {
  const [items, setItems] = useState<PendingAttachment[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [writing, setWriting] = useState(false);

  const disabled = opts?.disabled ?? false;
  const notAPicture = opts?.notAPicture ?? NOT_A_PICTURE;

  function attach(files: File[]) {
    if (!files.length || disabled) return;
    setError(null);
    setItems((a) => [...a, ...files.map(pendingFromFile)]);
  }

  /** From a drop; a non-image says so rather than reading as a dead target. */
  function attachPaths(paths: string[]) {
    if (disabled) return;
    const pictures = imagePaths(paths);
    if (!pictures.length) {
      if (paths.length) setError(notAPicture);
      return;
    }
    setError(null);
    setItems((a) => [...a, ...pictures.map(pendingFromPath)]);
  }

  function detach(id: string) {
    setItems((a) => {
      const gone = a.find((x) => x.id === id);
      if (gone) releaseAttachment(gone);
      return a.filter((x) => x.id !== id);
    });
  }

  const texts = useDraftStore((s) => s.pastes[draftKey] ?? NO_PASTES);
  const setPastes = useDraftStore((s) => s.setPastes);
  /** Read from the store, not the render: two edits in one tick both land. */
  const currentTexts = () => useDraftStore.getState().pastes[draftKey] ?? NO_PASTES;

  function attachText(text: string): boolean {
    if (!isLongPaste(text)) return false;
    setError(null);
    setPastes(draftKey, [...currentTexts(), pastedText(text)]);
    return true;
  }

  function editText(id: string, text: string) {
    setPastes(draftKey, currentTexts().map((p) => (p.id === id ? { ...p, text } : p)));
  }

  function detachText(id: string) {
    setPastes(draftKey, currentTexts().filter((p) => p.id !== id));
  }

  function takeBack(message: string): string {
    const { text, pasted } = splitPastedText(message);
    if (pasted.length) setPastes(draftKey, [...pasted.map(pastedText), ...currentTexts()]);
    return text;
  }

  const dropping = useFileDrop(ref, attachPaths);

  // Release blob URLs once, on unmount, via a ref to the latest list.
  const itemsRef = useRef(items);
  itemsRef.current = items;
  useEffect(() => () => itemsRef.current.forEach(releaseAttachment), []);

  async function prepare(text: string): Promise<string | null> {
    const key = draftKey;
    const words = withPastedText(text, currentTexts().map((p) => p.text));
    const pictures = itemsRef.current;
    if ((!words && !pictures.length) || writing) return null;
    if (!pictures.length) {
      setPastes(key, []);
      return words;
    }
    setWriting(true);
    let paths: string[];
    try {
      paths = await Promise.all(pictures.map(writeAttachment));
    } catch (e) {
      setError(String(e));
      return null;
    } finally {
      setWriting(false);
    }
    pictures.forEach(releaseAttachment);
    setItems([]);
    setPastes(key, []);
    setError(null);
    return withAttachments(words, paths);
  }

  return {
    items,
    error,
    writing,
    dropping,
    attach,
    detach,
    texts,
    attachText,
    editText,
    detachText,
    takeBack,
    prepare,
  };
}
