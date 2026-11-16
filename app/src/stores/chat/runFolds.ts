/** Which work folds the student opened or closed by hand, keyed by thread and
 *  fold (a run's `w-<run>`, or a stretch after a steer). Module state, not React state: a row leaving the mounted window (or a
 *  live run settling into its folded shape) must not forget the choice. */
const chosen = new Map<string, boolean>();

const key = (thread: number, fold: string) => `${thread}:${fold}`;

export function foldChoice(thread: number, fold: string): boolean | undefined {
  return chosen.get(key(thread, fold));
}

export function chooseFold(thread: number, fold: string, open: boolean): void {
  chosen.set(key(thread, fold), open);
}

/** A rewind deletes runs, and SQLite may hand their ids to the next ones. */
export function forgetFolds(thread: number): void {
  for (const k of chosen.keys()) if (k.startsWith(`${thread}:`)) chosen.delete(k);
}
