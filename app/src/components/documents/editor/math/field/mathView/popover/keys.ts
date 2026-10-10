/** Which list is open: the pending `\command`'s, or the picks Space opens. */
export type ListKind = "command" | "picks";

/**
 * What a key does to the open list: move the highlight, accept a row (by
 * index), open the full toolbox (`more`), close the list and take the key
 * (`close`), close it and let the key go on (`dismiss`), or nothing, the
 * key going on with the list open.
 */
export type ListKey = { move: number } | { accept: number } | "more" | "close" | "dismiss" | null;

type Keyish = Pick<KeyboardEvent, "key" | "shiftKey" | "metaKey" | "ctrlKey" | "altKey">;

const MODIFIER_KEYS = new Set(["Shift", "Meta", "Control", "Alt", "CapsLock", "Fn"]);

/** The highlight after a move: from none, ↓ takes the first row and ↑ the
 *  last; otherwise it wraps. */
export function moved(active: number, move: number, count: number): number {
  if (active < 0) return move > 0 ? 0 : count - 1;
  return (active + move + count) % count;
}

/**
 * The command list (first row highlighted) takes ↑/↓ and plain Space, Tab
 * and Enter, and after a bare `\` (`bare`) Esc, which closes it so Space
 * types `\ `; any other key is the model's, the list staying. The picks
 * (no row highlighted at first) take ↑/↓, Esc, 1–9, and Space, Tab and Enter
 * once a row is; Space with none opens the toolbox; any other key closes
 * them and goes on.
 */
export function listKey(e: Keyish, kind: ListKind, count: number, active: number, bare = false): ListKey {
  const mod = e.metaKey || e.ctrlKey || e.altKey;
  const plain = !mod && !e.shiftKey;
  if (kind === "command") {
    if (count === 0 || mod) return null;
    if (e.key === "Escape") return bare && !e.shiftKey ? "close" : null;
    if (e.key === "ArrowDown" && !e.shiftKey) return { move: 1 };
    if (e.key === "ArrowUp" && !e.shiftKey) return { move: -1 };
    if (plain && (e.key === " " || e.key === "Tab" || e.key === "Enter") && active >= 0) return { accept: active };
    return null;
  }
  if (MODIFIER_KEYS.has(e.key)) return null;
  if (mod) return "dismiss";
  if (e.key === "Escape") return "close";
  if (e.key === "ArrowDown" && !e.shiftKey && count) return { move: 1 };
  if (e.key === "ArrowUp" && !e.shiftKey && count) return { move: -1 };
  if (plain && e.key === " ") return active >= 0 ? { accept: active } : "more";
  if (plain && (e.key === "Tab" || e.key === "Enter") && active >= 0) return { accept: active };
  if (plain && /^[1-9]$/.test(e.key) && Number(e.key) <= count) return { accept: Number(e.key) - 1 };
  return "dismiss";
}
