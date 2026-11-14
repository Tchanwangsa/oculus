/**
 * The subject a download most likely belongs to: a Canvas course page names
 * it in `/courses/<id>/`, and a subject's id is its Canvas course id. Read
 * against the institution's own Canvas address when it is known, so another
 * site's `/courses/12/` path is not taken for one.
 */

/** The Canvas course id in `url`'s path, or null. */
export function courseIdOf(url: string): number | null {
  let path: string;
  try {
    path = new URL(url).pathname;
  } catch {
    return null;
  }
  const match = /\/courses\/(\d+)(?:\/|$)/.exec(path);
  return match ? Number(match[1]) : null;
}

function sameHost(url: string, canvas: string): boolean {
  try {
    return new URL(url).host === new URL(canvas).host;
  } catch {
    return false;
  }
}

/**
 * The code of the subject the tab's `url` is a course page of, if any.
 * `canvas` is the profile's `https://host`, or null before one is set.
 */
export function guessSubject(
  url: string | null | undefined,
  subjects: readonly { id: number; code: string }[],
  canvas: string | null,
): string | null {
  if (!url) return null;
  if (canvas && !sameHost(url, canvas)) return null;
  const id = courseIdOf(url);
  if (id == null) return null;
  return subjects.find((s) => s.id === id)?.code ?? null;
}

/** The dialog's preselected subject: the guess, else the last one chosen if
 *  it still exists. */
export function preselectSubject(
  guess: string | null,
  last: string | null,
  subjects: readonly { code: string }[],
): string | null {
  if (guess) return guess;
  return last && subjects.some((s) => s.code === last) ? last : null;
}
