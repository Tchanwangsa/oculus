/** What a key does to the `\command` list: move the highlight, accept it,
 *  or nothing (the model takes the key). */
export type ListKey = { move: number } | "accept" | null;

/** ↑/↓ move, plain Space, Tab and Enter accept; only while the list has
 *  options, so an unknown name still commits as typed. */
export function listKey(
  e: Pick<KeyboardEvent, "key" | "shiftKey" | "metaKey" | "ctrlKey" | "altKey">,
  count: number,
): ListKey {
  if (count === 0 || e.metaKey || e.ctrlKey || e.altKey) return null;
  switch (e.key) {
    case "ArrowDown":
      return e.shiftKey ? null : { move: 1 };
    case "ArrowUp":
      return e.shiftKey ? null : { move: -1 };
    case " ":
    case "Tab":
    case "Enter":
      return e.shiftKey ? null : "accept";
  }
  return null;
}
