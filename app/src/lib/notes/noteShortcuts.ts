/** The menu's ⌘K stays synchronous without loading the note editor. Each
 *  mounted editor owns its callback; removing it releases the captured view. */
const linkCommands = new WeakMap<Element, () => boolean>();

export function registerNoteLinkCommand(root: Element, command: () => boolean): () => void {
  linkCommands.set(root, command);
  return () => {
    if (linkCommands.get(root) === command) linkCommands.delete(root);
  };
}

export function linkInFocusedNote(): boolean {
  const root = document.activeElement?.closest(".cm-editor");
  return root ? linkCommands.get(root)?.() ?? false : false;
}
