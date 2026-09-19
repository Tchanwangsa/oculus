import { useEffect, useRef, useState } from "react";

import { useFileDrop } from "@/hooks/useFileDrop";
import {
  imagePaths,
  pendingFromFile,
  pendingFromPath,
  releaseAttachment,
  writeAttachment,
  type PendingAttachment,
} from "@/lib/attachments";

/** What a drop of something that is not an image says. The page composer's
 *  wording, because that box has the `@` menu the sentence points at; a box
 *  without one passes its own. */
const NOT_A_PICTURE = "Only images can be attached — use @ for a course file.";

export interface Attachments {
  /** Pictures waiting to go out, in the order they arrived. */
  items: PendingAttachment[];
  /** Why one could not be attached or written. Never a reason to lose the
   *  message — the box keeps what was typed and says this above it. */
  error: string | null;
  setError: (e: string | null) => void;
  /** True while a send's pictures are being written. */
  writing: boolean;
  /** True while a drag is over the box, for the caller to draw. */
  dropping: boolean;
  /** Pictures from a paste, which arrive as `File`s the clipboard owns. */
  attach: (files: File[]) => void;
  detach: (id: string) => void;
  /**
   * Write everything pending and answer the paths, or `null` if a write
   * failed — in which case the error is already set and the caller must keep
   * the box exactly as it was. An empty list is a successful `[]`, so a
   * message with no pictures takes the same path as one with them.
   */
  flush: () => Promise<string[] | null>;
}

/**
 * Pictures on their way into a message: pasted, dropped, drawn as a strip,
 * written on send.
 *
 * **Every composer gets this, and gets it the same way.** The boxes differ —
 * the page's is wide and has an `@` menu, the lecture dock's is 300px beside a
 * playing video — but a screenshot dragged onto either one is the same gesture
 * asking for the same thing, and a box that quietly ignores it reads as
 * broken. What varies is only the wording of a refusal and the size of a chip,
 * so those are arguments and the rest is here.
 *
 * Nothing touches the disk until send (`writeAttachment`,
 * `app/src/lib/attachments.ts`): a screenshot pasted and then thought better
 * of leaves nothing behind. A task *body* is the one editor that cannot work
 * this way — there is no send to defer to — which is why `TaskPage` writes on
 * arrival and keeps its own handlers rather than taking this.
 *
 * The drop half is `useFileDrop`, which is where the hard-won parts live: the
 * listener is the **webview's** and not the window's, and the position it
 * reports is in points however it is typed.
 */
export function useAttachments(
  ref: React.RefObject<HTMLElement | null>,
  opts?: {
    /** What a drop of a non-image says, when the default sentence points at
     *  something this box does not have. */
    notAPicture?: string;
    /** Refuse drops entirely — an editor that is not open, say. Paste is the
     *  caller's own to withhold. */
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

  /**
   * …and from a drop, which arrives as paths.
   *
   * A drop of something that is not an image says so rather than being
   * ignored: a dropped PDF is a reasonable thing to try, and silence would
   * read as a broken drop target.
   */
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

  // Blob URLs outlive the component unless they are let go of. The list is
  // read through a ref so the cleanup runs once, on unmount, rather than on
  // every change to it.
  const itemsRef = useRef(items);
  itemsRef.current = items;
  useEffect(() => () => itemsRef.current.forEach(releaseAttachment), []);

  async function flush(): Promise<string[] | null> {
    const pictures = itemsRef.current;
    if (!pictures.length) return [];
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
    return paths;
  }

  return { items, error, setError, writing, dropping, attach, detach, flush };
}
