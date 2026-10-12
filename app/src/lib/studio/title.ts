const MAX = 60;

/** A working title from the words a video was asked for: the first line
 *  without its `@` paths, cut at a word. The thread's own title is named
 *  later, so this only has to make a readable slug. */
export function titleFromPrompt(text: string): string {
  const line = text.split("\n").find((l) => l.trim()) ?? "";
  const words = line.split(/\s+/).filter((w) => w && !w.includes("/") && !w.startsWith("@"));
  let title = "";
  for (const w of words) {
    if (`${title} ${w}`.trim().length > MAX) break;
    title = `${title} ${w}`.trim();
  }
  return title.replace(/[.,;:!?]+$/, "") || "Untitled video";
}
