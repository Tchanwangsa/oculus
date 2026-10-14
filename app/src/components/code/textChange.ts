/** The one span that turns `before` into `after`: their common start and end
 *  kept, the middle replaced. Replaying another copy's text this way leaves a
 *  caret outside the changed span where it was. */
export function textChange(before: string, after: string): { from: number; to: number; insert: string } {
  const max = Math.min(before.length, after.length);
  let start = 0;
  while (start < max && before.charCodeAt(start) === after.charCodeAt(start)) start++;
  let end = 0;
  while (
    end < max - start &&
    before.charCodeAt(before.length - 1 - end) === after.charCodeAt(after.length - 1 - end)
  ) {
    end++;
  }
  return { from: start, to: before.length - end, insert: after.slice(start, after.length - end) };
}
