/** Formulas the field trapped on, by display flag and source: they stay TeX
 *  (`readsCleanly`) rather than reopen and trap again. */
const trapped = new Set<string>();
const TRAPPED_MAX = 200;

export function markFieldTrap(source: string, display: boolean) {
  if (trapped.size >= TRAPPED_MAX) trapped.clear();
  trapped.add(`${display ? "D" : "I"}${source}`);
}

export function fieldTrapped(source: string, display: boolean): boolean {
  return trapped.has(`${display ? "D" : "I"}${source}`);
}
