/** The indent unit when a file shows none: Ed's, and PEP 8's. */
export const DEFAULT_INDENT = "    ";

/** Lines read to guess; a file's indent shows well before this. */
const SAMPLE_LINES = 2000;

/**
 * The indent unit a file uses: a tab when most indented lines start with
 * one, else the step between a line and a deeper next line seen most often
 * (2–8 spaces; a tie goes to 4, then the narrower). Blank lines are skipped,
 * so a step is measured across them. Without indented lines, 4 spaces.
 */
export function detectIndent(text: string): string {
  const lines = text.split("\n", SAMPLE_LINES);
  let tabs = 0;
  let spaced = 0;
  const steps = new Map<number, number>();
  let previous = 0;
  for (const line of lines) {
    if (!line.trim()) continue;
    const lead = /^[ \t]*/.exec(line)![0];
    if (lead.startsWith("\t")) {
      tabs += 1;
      continue;
    }
    const width = lead.length;
    if (width > 0) spaced += 1;
    const step = width - previous;
    if (step >= 2 && step <= 8) steps.set(step, (steps.get(step) ?? 0) + 1);
    previous = width;
  }
  if (tabs > spaced) return "\t";
  let best = 0;
  let bestCount = 0;
  for (const [step, count] of steps) {
    const better =
      count > bestCount ||
      (count === bestCount && (step === 4 || (best !== 4 && step < best)));
    if (better) {
      best = step;
      bestCount = count;
    }
  }
  return best ? " ".repeat(best) : DEFAULT_INDENT;
}
