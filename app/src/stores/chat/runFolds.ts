/** Which work folds the student opened or closed by hand, keyed by thread and
 *  run. Module state, not React state: a row leaving the mounted window (or a
 *  live run settling into its folded shape) must not forget the choice. */
const chosen = new Map<string, boolean>();

const key = (thread: number, run: number) => `${thread}:${run}`;

export function foldChoice(thread: number, run: number): boolean | undefined {
  return chosen.get(key(thread, run));
}

export function chooseFold(thread: number, run: number, open: boolean): void {
  chosen.set(key(thread, run), open);
}

/** A rewind deletes runs, and SQLite may hand their ids to the next ones. */
export function forgetFolds(thread: number): void {
  for (const k of chosen.keys()) if (k.startsWith(`${thread}:`)) chosen.delete(k);
}
