const storageKey = (subjectId: number) => `oculus:overview:last-visit:${subjectId}`;

/** Epoch milliseconds keep device-local visits independent of source date formats. */
export function readVisit(subjectId: number): number | null {
  try {
    const value = localStorage.getItem(storageKey(subjectId));
    const ms = value == null || value.trim() === "" ? NaN : Number(value);
    return Number.isFinite(ms) && ms >= 0 ? ms : null;
  } catch {
    return null;
  }
}

export function writeVisit(subjectId: number, now: number): void {
  if (!Number.isFinite(now) || now < 0) return;
  try {
    localStorage.setItem(storageKey(subjectId), String(Math.max(readVisit(subjectId) ?? 0, now)));
  } catch {
    // Storage can be unavailable; the overview remains usable without visit dots.
  }
}
