/** The shortest target a video's length can be asked for, in seconds. */
export const MIN_LENGTH_SECS = 5;
/** Upper bound of a typed length: keeps the number a sane integer for Rust's `u32`. */
const MAX_LENGTH_SECS = 24 * 3600;

export type ParsedLength = { secs: number } | { problem: string };

const UNITS: Record<string, number> = {
  "": 1,
  s: 1,
  sec: 1,
  secs: 1,
  second: 1,
  seconds: 1,
  m: 60,
  min: 60,
  mins: 60,
  minute: 60,
  minutes: 60,
};

/**
 * Reads a typed length into whole seconds: `45` and `90s` are seconds, `3m`
 * and `1.5 min` are minutes, `1:30` is minutes and seconds. Anything else, or
 * a length under five seconds, gives a sentence fit to show beside the field.
 */
export function parseLength(text: string): ParsedLength {
  const t = text.trim().toLowerCase();
  const bad = { problem: "Type a length like 45, 90s, 3m or 1:30." };
  if (!t) return bad;
  let secs: number;
  const clock = /^(\d+):([0-5]?\d)$/.exec(t);
  if (clock) {
    secs = Number(clock[1]) * 60 + Number(clock[2]);
  } else {
    const m = /^(\d+(?:\.\d+)?|\.\d+)\s*([a-z]*)$/.exec(t);
    const unit = m ? UNITS[m[2]] : undefined;
    if (!m || unit === undefined) return bad;
    secs = Number(m[1]) * unit;
  }
  secs = Math.round(secs);
  if (!Number.isFinite(secs) || secs < MIN_LENGTH_SECS) {
    return { problem: `A video needs at least ${MIN_LENGTH_SECS} seconds.` };
  }
  if (secs > MAX_LENGTH_SECS) return { problem: "That is longer than a day." };
  return { secs };
}

/** A length as the style chip prints it: `2 min`, `90 s`, `2:30`. */
export function formatLength(secs: number): string {
  if (secs < 120) return `${secs} s`;
  const m = Math.floor(secs / 60);
  const s = secs % 60;
  return s === 0 ? `${m} min` : `${m}:${String(s).padStart(2, "0")}`;
}

/** A length as the custom field shows it for editing; `parseLength` reads it back. */
export function lengthInput(secs: number): string {
  return secs >= 120 && secs % 60 === 0 ? `${secs / 60}m` : `${secs}s`;
}
