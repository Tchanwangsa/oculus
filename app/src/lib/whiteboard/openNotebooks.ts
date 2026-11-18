/**
 * The notebooks open in the editor right now. A chat message names them (`openNotebookContext`) so
 * "is my working on page 2 right?" needs no `@`: the agent is told which notebook, which page is in view,
 * and how to read its pages.
 */
export interface OpenNotebook { path: string; title: string; pages: number; page: number }

const open = new Map<number, OpenNotebook & { touched: number }>();
let clock = 0;

/** Lists a notebook as open until the returned function is called. */
export function registerOpenNotebook(fileId: number, notebook: OpenNotebook): () => void {
  open.set(fileId, { ...notebook, touched: ++clock });
  return () => { open.delete(fileId); };
}

/** Records the page now in view; the notebook counts as the one the student touched last. */
export function noteNotebookPage(fileId: number, page: number): void {
  const current = open.get(fileId);
  if (current) open.set(fileId, { ...current, page, touched: ++clock });
}

const MAX_LISTED = 3;
const quoted = (path: string) => `"${path.replace(/(["\\$`])/g, '\\$1')}"`;

/** What to add to a chat message while a notebook is open, or null. */
export function openNotebookContext(): string | null {
  const notebooks = [...open.values()].sort((a, b) => b.touched - a.touched).slice(0, MAX_LISTED);
  if (!notebooks.length) return null;
  const lines = notebooks.map(n => `- “${n.title}”, page ${n.page} of ${n.pages} in view: \`oculus read ${quoted(n.path)}\``);
  return [
    notebooks.length === 1 ? 'The student has a notebook open in Oculus, where they write their working by hand:' : 'The student has these notebooks open in Oculus, where they write their working by hand (the first was used last):',
    ...lines,
    'That command lists the notebook\'s page pictures, redrawn a few seconds after each stroke. Open the page you need with your image tool and respond to what is written there; if they ask about "my working", "my solution" or "the canvas", this is it.',
  ].join('\n');
}
