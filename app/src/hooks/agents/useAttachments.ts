import { useEffect, useRef, useState, type RefObject } from "react";

import type { MentionInputHandle } from "@/components/harness/mentions/types";
import { useDataDir } from "@/hooks/backend/useDataDir";
import { useFileDrop } from "@/hooks/gestures/useFileDrop";
import {
  appendAttachmentRefs,
  attachmentPath,
  attachmentSrc,
  distinctPaths,
  isImagePath,
  isLongPaste,
  pastedText,
  pendingFromFile,
  pendingFromPath,
  refersTo,
  releaseAttachment,
  splitHandedBack,
  withPastedText,
  withShelf,
  writeAttachment,
  type PastedText,
  type PendingAttachment,
} from "@/lib/harness/attachments";
import { attachmentKindLabel, useAttachmentNames } from "@/stores/chat/attachmentNames";
import { useDraftStore } from "@/stores/chat/draftStore";

/** Stable empty lists, so the store selectors don't re-render forever. */
const NO_PASTES: PastedText[] = [];
const NO_SHELF: string[] = [];

/** A picture on a draft's shelf, already in `agents/attachments/`. */
export interface ShelfItem {
  /** Its `./attachments/…` path, which is also its identity. */
  id: string;
  name: string;
  preview: string;
}

export interface Attachments {
  /** The shelf: pictures attached to this draft, chip or no chip. */
  items: ShelfItem[];
  error: string | null;
  /** A file is being written; a send waits for its chip. */
  writing: boolean;
  dropping: boolean;
  /** From a paste. */
  attach: (files: File[]) => void;
  /** A thumbnail's remove: at once if no chip names it, else asks first. */
  detach: (id: string) => void;
  /** The shelf picture whose removal is waiting on "remove it there too". */
  confirming: string | null;
  confirmDetach: () => void;
  cancelDetach: () => void;
  /** Long pastes held as cards, kept in `draftStore` under the draft key. */
  texts: PastedText[];
  /** Takes a long plain-text paste as a card; false leaves it to the box. */
  attachText: (text: string) => boolean;
  editText: (id: string, text: string) => void;
  detachText: (id: string) => void;
  /** Lifts the pasted blocks and pictures out of handed-back words (a stop, a
   *  rewind) into cards and shelf pictures ahead of any held, returning the
   *  rest, chips' paths in place, for the box. */
  takeBack: (message: string) => string;
  /** The message to send: chips' paths already stand in the text, pasted
   *  cards are added as blocks and a shelf picture with no chip on the
   *  trailing line. Null (and the draft kept) when there is nothing, or a file
   *  is still being written. */
  prepare: (text: string) => string | null;
  /** Takes the shelf and pasted cards out for the stash; null while a file is
   *  being written. */
  setAside: () => { pastes: string[]; pictures: string[] } | null;
  /** A stash entry's pastes as cards and its pictures on the shelf, after
   *  any already held. */
  bringBack: (stashed: { pastes: string[]; pictures: string[] }) => void;
}

/** A draft's shelf, read from the store rather than the render, so two edits
 *  in one tick both land. */
const shelfOf = (key: string) => useDraftStore.getState().shelves[key] ?? NO_SHELF;
const textOf = (key: string) => useDraftStore.getState().drafts[key] ?? "";

/**
 * Files on their way into a message, for both chat composers: pasted or
 * dropped (`useFileDrop`), each written to `agents/attachments/` the moment
 * it arrives and put into the sentence as a chip at the caret, through the
 * box's handle (`input`). A picture also joins the draft's shelf of
 * thumbnails (`draftStore`), which outlives the box; a PDF or text file is
 * only its chip. A write that lands after the composer moved to another
 * draft goes into that draft's text instead. Long pastes ride along as text
 * cards under `draftKey`, going into the message as `<pasted_text>` blocks.
 */
export function useAttachments(
  ref: RefObject<HTMLElement | null>,
  draftKey: string,
  opts: {
    /** The box the chips go into. */
    input: RefObject<MentionInputHandle | null>;
    /** Refuse drops; paste is the caller's own to withhold. */
    disabled?: boolean;
  },
): Attachments {
  const [error, setError] = useState<string | null>(null);
  const [writes, setWrites] = useState(0);
  const [confirming, setConfirming] = useState<string | null>(null);
  const dataDir = useDataDir();
  const disabled = opts.disabled ?? false;
  const input = opts.input;

  const keyRef = useRef(draftKey);
  keyRef.current = draftKey;
  useEffect(() => setConfirming(null), [draftKey]);

  const shelf = useDraftStore((s) => s.shelves[draftKey] ?? NO_SHELF);
  const setShelf = useDraftStore((s) => s.setShelf);
  const names = useAttachmentNames((s) => s.names);

  const items: ShelfItem[] = shelf.map((path) => {
    const rel = attachmentPath(path) ?? path;
    return { id: path, name: names[rel] ?? attachmentKindLabel(rel), preview: attachmentSrc(dataDir, rel) };
  });

  /** Chips for the written paths: into the box when it still shows `key`,
   *  else after that draft's text. Pictures join its shelf either way. */
  function land(key: string, paths: string[]) {
    const pictures = paths.filter(isImagePath);
    if (pictures.length) setShelf(key, distinctPaths([...shelfOf(key), ...pictures]));
    const box = input.current;
    if (box && key === keyRef.current) box.insertAttachments(paths);
    else useDraftStore.getState().setDraft(key, appendAttachmentRefs(textOf(key), paths));
  }

  async function add(pending: PendingAttachment[]) {
    if (!pending.length || disabled) return;
    const key = draftKey;
    setError(null);
    setWrites((n) => n + 1);
    const results = await Promise.allSettled(pending.map(writeAttachment));
    pending.forEach(releaseAttachment);
    setWrites((n) => n - 1);
    const paths: string[] = [];
    results.forEach((r, i) => {
      if (r.status !== "fulfilled") return;
      paths.push(r.value);
      useAttachmentNames.getState().name(r.value, pending[i].name);
    });
    const failed = results.find((r) => r.status === "rejected");
    if (failed) setError(String(failed.reason));
    if (paths.length) land(key, paths);
  }

  const attach = (files: File[]) => void add(files.map(pendingFromFile));
  /** From a drop: Rust sniffs each and says what it refused. */
  const attachPaths = (paths: string[]) => void add(paths.map(pendingFromPath));

  function removeFromShelf(path: string) {
    setShelf(draftKey, shelfOf(draftKey).filter((p) => p !== path));
  }

  function detach(path: string) {
    if (refersTo(textOf(draftKey), path)) setConfirming(path);
    else removeFromShelf(path);
  }

  function confirmDetach() {
    if (!confirming) return;
    removeFromShelf(confirming);
    input.current?.removeAttachment(confirming);
    setConfirming(null);
  }

  const texts = useDraftStore((s) => s.pastes[draftKey] ?? NO_PASTES);
  const setPastes = useDraftStore((s) => s.setPastes);
  const currentTexts = () => useDraftStore.getState().pastes[draftKey] ?? NO_PASTES;

  function attachText(pasted: string): boolean {
    if (!isLongPaste(pasted)) return false;
    setError(null);
    setPastes(draftKey, [...currentTexts(), pastedText(pasted)]);
    return true;
  }

  function editText(id: string, next: string) {
    setPastes(draftKey, currentTexts().map((p) => (p.id === id ? { ...p, text: next } : p)));
  }

  function detachText(id: string) {
    setPastes(draftKey, currentTexts().filter((p) => p.id !== id));
  }

  function takeBack(message: string): string {
    const back = splitHandedBack(message);
    if (back.pictures.length) setShelf(draftKey, distinctPaths([...back.pictures, ...shelfOf(draftKey)]));
    if (back.pasted.length) setPastes(draftKey, [...back.pasted.map(pastedText), ...currentTexts()]);
    return back.text;
  }

  const dropping = useFileDrop(ref, attachPaths);

  function prepare(typed: string): string | null {
    if (writes > 0) return null;
    const words = withPastedText(typed, currentTexts().map((p) => p.text));
    const message = withShelf(words, shelfOf(draftKey));
    if (!message) return null;
    setPastes(draftKey, []);
    setShelf(draftKey, []);
    setError(null);
    setConfirming(null);
    return message;
  }

  function setAside(): { pastes: string[]; pictures: string[] } | null {
    if (writes > 0) return null;
    const pictures = shelfOf(draftKey);
    const pastes = currentTexts().map((p) => p.text);
    setShelf(draftKey, []);
    if (pastes.length) setPastes(draftKey, []);
    setError(null);
    setConfirming(null);
    return { pastes, pictures };
  }

  function bringBack(stashed: { pastes: string[]; pictures: string[] }) {
    const pictures = stashed.pictures.filter((p) => attachmentPath(p));
    if (pictures.length) setShelf(draftKey, distinctPaths([...shelfOf(draftKey), ...pictures]));
    if (stashed.pastes.length) setPastes(draftKey, [...currentTexts(), ...stashed.pastes.map(pastedText)]);
  }

  return {
    items,
    error,
    writing: writes > 0,
    dropping,
    attach,
    detach,
    confirming,
    confirmDetach,
    cancelDetach: () => setConfirming(null),
    texts,
    attachText,
    editText,
    detachText,
    takeBack,
    prepare,
    setAside,
    bringBack,
  };
}
