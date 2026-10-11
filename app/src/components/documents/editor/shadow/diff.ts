const AROUND = 40;

/** Where two strings first differ, each cut to `AROUND` units either side, so
 *  a report never carries a whole note twice. */
export function firstDifference(expected: string, actual: string) {
  let at = 0;
  while (at < expected.length && at < actual.length && expected[at] === actual[at]) at++;
  const cut = (s: string) => ({
    length: s.length,
    excerpt: `${at > AROUND ? "…" : ""}${s.slice(Math.max(0, at - AROUND), at + AROUND)}${at + AROUND < s.length ? "…" : ""}`,
  });
  return { at, expected: cut(expected), actual: cut(actual) };
}
