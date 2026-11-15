import { useState, type KeyboardEvent, type RefObject } from "react";
import type { MentionInputHandle } from "@/components/harness/mentions/MentionInput";
import type { Attachments } from "@/hooks/agents/useAttachments";
import { useStashStore, type StashEntry } from "@/stores/chat/stashStore";

/** ⌘S (Ctrl+S): the stash's key, on the composer's own keydown. */
export function isStashKey(e: { key: string; metaKey: boolean; ctrlKey: boolean; shiftKey: boolean; altKey: boolean }): boolean {
  return (e.metaKey || e.ctrlKey) && !e.shiftKey && !e.altKey && e.key.toLowerCase() === "s";
}

/**
 * The composer's side of the stash (`stashStore`): ⌘S with anything in the box
 * sets it aside, chips and all (their files are already written); ⌘S on an
 * empty box restores the only
 * entry or opens the menu. Handled on the box's own keydown and stopped there,
 * so a note open in another pane, which takes ⌘S on `document`, never sees it.
 */
export function useComposerStash({
  input,
  text,
  att,
}: {
  input: RefObject<MentionInputHandle | null>;
  /** The box's serialized message. */
  text: string;
  att: Attachments;
}) {
  const entries = useStashStore((s) => s.entries);
  const evicted = useStashStore((s) => s.evicted);
  const [open, setOpenState] = useState(false);

  function setOpen(next: boolean) {
    setOpenState(next);
    if (!next) useStashStore.getState().clearEvicted();
  }

  const holding = text.trim().length > 0 || att.items.length > 0 || att.texts.length > 0;

  function stash() {
    const aside = att.setAside();
    if (!aside) return;
    useStashStore.getState().stash({ text, ...aside });
    input.current?.clear();
  }

  /** Back into the box: words after any typed with their chips, pastes as
   *  cards, pictures on the shelf; the entry leaves the stash. */
  function restore(entry: StashEntry) {
    useStashStore.getState().remove(entry.id);
    att.bringBack(entry);
    // Through `replace`, so the entry's mentions and skills come back as chips.
    if (entry.text.trim()) input.current?.replace(text.trim() ? `${text.trimEnd()}\n\n${entry.text}` : entry.text);
    setOpen(false);
  }

  function keyDown(e: KeyboardEvent<HTMLElement>) {
    if (!isStashKey(e)) return;
    e.preventDefault();
    e.stopPropagation();
    if (holding) stash();
    else if (entries.length === 1) restore(entries[0]);
    else if (entries.length > 1) setOpen(true);
  }

  return {
    keyDown,
    /** Spread into `<StashMenu {...menu} />`. */
    menu: {
      entries,
      evicted,
      open,
      onOpenChange: setOpen,
      onRestore: restore,
      onDelete: (entry: StashEntry) => useStashStore.getState().remove(entry.id),
    },
  };
}
