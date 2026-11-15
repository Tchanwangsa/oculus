import { splitAttachments, splitPastedText, stripAttachmentRefs } from "./attachments";

/**
 * Recalling earlier messages into an empty composer with ArrowUp and
 * ArrowDown, after T3 Code's `composerPromptHistory.ts`: only what the
 * student typed, repeats collapsed, walked back and forward.
 */

/** A message the student sent, by its row. */
export interface PromptHistoryEntry {
  id: number;
  prompt: string;
}

/** Which entry the box shows, and the text it was given, so an edit to the
 *  recalled text ends the walk. */
export interface PromptHistoryPosition {
  entryId: number;
  recalled: string;
}

/** A sent message as the box would take it back: its pasted blocks, its
 *  line of picture paths and every attachment chip in its sentence are
 *  dropped (they were cards, thumbnails and files, not words, as T3 strips
 *  its references), library mentions and skills stay. */
export function recallablePrompt(message: string): string {
  const { text: unpictured } = splitAttachments(message);
  return stripAttachmentRefs(splitPastedText(unpictured).text).trim();
}

/** The thread's user rows, oldest first, as entries: blank ones (a message of
 *  only pictures or pastes) skipped, a repeat of the one before collapsed
 *  into the later. */
export function promptHistoryEntries(rows: readonly { id: number; kind: string; content: string | null }[]): PromptHistoryEntry[] {
  const entries: PromptHistoryEntry[] = [];
  for (const row of rows) {
    if (row.kind !== "user" || !row.content) continue;
    const prompt = recallablePrompt(row.content);
    if (!prompt) continue;
    const previous = entries[entries.length - 1];
    if (previous?.prompt === prompt) entries[entries.length - 1] = { id: row.id, prompt };
    else entries.push({ id: row.id, prompt });
  }
  return entries;
}

function findActive(entries: readonly PromptHistoryEntry[], position: PromptHistoryPosition): number {
  const byId = entries.findIndex((e) => e.id === position.entryId);
  if (byId >= 0) return byId;
  for (let i = entries.length - 1; i >= 0; i--) if (entries[i].prompt === position.recalled) return i;
  return -1;
}

/** One step through the history, or null when the key is the editor's: back
 *  only from an empty box or a recalled entry left as it was, forward only
 *  from a recalled entry. Forward past the newest empties the box. */
export function stepPromptHistory(input: {
  direction: "backward" | "forward";
  entries: readonly PromptHistoryEntry[];
  position: PromptHistoryPosition | null;
  current: string;
}): { position: PromptHistoryPosition | null; prompt: string } | null {
  const { entries, position, current } = input;
  const active = position && position.recalled === current ? findActive(entries, position) : -1;

  if (input.direction === "backward") {
    if (active < 0 && current.trim().length > 0) return null;
    const entry = entries[active < 0 ? entries.length - 1 : active - 1];
    if (!entry) return null;
    return { position: { entryId: entry.id, recalled: entry.prompt }, prompt: entry.prompt };
  }

  if (active < 0) return null;
  const entry = entries[active + 1];
  if (!entry) return { position: null, prompt: "" };
  return { position: { entryId: entry.id, recalled: entry.prompt }, prompt: entry.prompt };
}
