const LAST_SUBJECT_KEY = "oculus-revision-subject";

/** The subject Revision last showed, so returning to it keeps the scope. */
export function lastRevisionSubject(): number | null {
  try {
    const id = Number(localStorage.getItem(LAST_SUBJECT_KEY));
    return Number.isInteger(id) && id > 0 ? id : null;
  } catch {
    return null;
  }
}

export function rememberRevisionSubject(id: number): void {
  try {
    localStorage.setItem(LAST_SUBJECT_KEY, String(id));
  } catch {
    /* quota or private mode — Revision falls back to the first subject */
  }
}
