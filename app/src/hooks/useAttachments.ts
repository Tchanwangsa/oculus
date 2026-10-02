import { useEffect, useRef, useState } from "react";

import { useFileDrop } from "@/hooks/useFileDrop";
import {
  imagePaths,
  pendingFromFile,
  pendingFromPath,
  releaseAttachment,
  writeAttachment,
  withAttachments,
  type PendingAttachment,
} from "@/lib/attachments";

/** Points at the `@` menu; a box without one passes its own wording. */
const NOT_A_PICTURE = "Only images can be attached — use @ for a course file.";

export interface Attachments {
  items: PendingAttachment[];
  error: string | null;
  writing: boolean;
  dropping: boolean;
  /** From a paste. */
  attach: (files: File[]) => void;
  detach: (id: string) => void;
  /** Prepare a trimmed message and its attachments; null keeps the editor intact. */
  prepare: (text: string) => Promise<string | null>;
}

/** Pictures on their way into a message, for every composer: pasted or
 *  dropped (`useFileDrop`), written to disk only on send. `TaskPage` writes on
 *  arrival instead, having no send to defer to. */
export function useAttachments(
  ref: React.RefObject<HTMLElement | null>,
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

  const dropping = useFileDrop(ref, attachPaths);

  // Release blob URLs once, on unmount, via a ref to the latest list.
  const itemsRef = useRef(items);
  itemsRef.current = items;
  useEffect(() => () => itemsRef.current.forEach(releaseAttachment), []);

  async function prepare(text: string): Promise<string | null> {
    const trimmed = text.trim();
    const pictures = itemsRef.current;
    if ((!trimmed && !pictures.length) || writing) return null;
    if (!pictures.length) return trimmed;
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
    setError(null);
    return withAttachments(trimmed, paths);
  }

  return { items, error, writing, dropping, attach, detach, prepare };
}
