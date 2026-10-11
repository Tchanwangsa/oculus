import { MATH_COMMANDS, MATH_TABS, POPULAR_DEFAULTS, type MathEntry } from "./mathPalette";

/**
 * What the maths toolbox remembers between sessions: the last few entries
 * used (its Recent row), how often each was used (its Popular tab), and the
 * last few used in each subject (which lead the visual field's list). A
 * palette click, an accepted `\` completion and a `\command` typed out in
 * TeX or in the visual field all count; a typed command counts as its
 * palette entry, so its cell renders and inserts like one. All live in
 * `localStorage`, and a blocked or full store just stops remembering.
 */

const RECENTS_KEY = "oculus-math-recents";
const RECENTS_MAX = 8;
const USAGE_KEY = "oculus-math-usage";
/** Recents per subject, keyed by subject id (`none` for a note without one). */
const SUBJECT_RECENTS_KEY = "oculus-math-subject-recents";
/** Entries kept in the usage map; the least used go first. */
const USAGE_MAX = 120;
/** Grid cells the Popular tab fills: three rows of eight, what shows
 *  without scrolling (a wide entry takes two). */
const POPULAR_CELLS = 24;

/** Bumped on every use, so an open toolbox knows to redraw its Recent row. */
let version = 0;

export function usageVersion(): number {
  return version;
}

const kept = (e: MathEntry): MathEntry => ({ template: e.template, label: e.label, wide: e.wide });

const entries = (list: unknown): MathEntry[] =>
  Array.isArray(list) ? list.filter((e): e is MathEntry => typeof e?.template === "string") : [];

export function readRecents(): MathEntry[] {
  try {
    return entries(JSON.parse(localStorage.getItem(RECENTS_KEY) ?? "[]")).slice(0, RECENTS_MAX);
  } catch {
    return [];
  }
}

const subjectKey = (subject: number | null) => (subject == null ? "none" : String(subject));

function readSubjectRecents(): Record<string, unknown> {
  try {
    const map: unknown = JSON.parse(localStorage.getItem(SUBJECT_RECENTS_KEY) ?? "{}");
    return map && typeof map === "object" && !Array.isArray(map) ? (map as Record<string, unknown>) : {};
  } catch {
    return {};
  }
}

/** The entries last used in `subject`'s notes, most recent first. */
export function subjectRecents(subject: number | null): MathEntry[] {
  return entries(readSubjectRecents()[subjectKey(subject)]).slice(0, SUBJECT_RECENTS);
}

interface Usage extends MathEntry {
  /** Times used. */
  n: number;
  /** Last used, ms since the epoch. */
  t: number;
}

function readUsage(): Usage[] {
  try {
    const list: unknown = JSON.parse(localStorage.getItem(USAGE_KEY) ?? "[]");
    if (!Array.isArray(list)) return [];
    return list.filter(
      (e): e is Usage => typeof e?.template === "string" && typeof e.n === "number" && typeof e.t === "number",
    );
  } catch {
    return [];
  }
}

/** Most used first, the more recent of a tie ahead. */
const byUse = (a: Usage, b: Usage) => b.n - a.n || b.t - a.t;

/** `entry` at the front of `list`, once. */
const toFront = (entry: MathEntry, list: MathEntry[], max: number) =>
  [kept(entry), ...list.filter((e) => e.template !== entry.template)].slice(0, max);

/** One use of `entry` in a note of `subject`: to the front of the recents
 *  and the subject's, one more in the counts. */
export function recordUse(entry: MathEntry, subject: number | null) {
  version++;
  const recents = toFront(entry, readRecents(), RECENTS_MAX);
  const bySubject = readSubjectRecents();
  bySubject[subjectKey(subject)] = toFront(entry, subjectRecents(subject), SUBJECT_RECENTS);
  const usage = readUsage();
  const prev = usage.find((e) => e.template === entry.template);
  const next: Usage = { ...kept(entry), n: (prev?.n ?? 0) + 1, t: Date.now() };
  const counts = [next, ...usage.filter((e) => e !== prev)].sort(byUse).slice(0, USAGE_MAX);
  try {
    localStorage.setItem(RECENTS_KEY, JSON.stringify(recents));
    localStorage.setItem(USAGE_KEY, JSON.stringify(counts));
    localStorage.setItem(SUBJECT_RECENTS_KEY, JSON.stringify(bySubject));
  } catch {
    // Storage full or blocked: nothing persists.
  }
}

/** The `\command` (or `\begin{env}`) a template starts with. */
export function commandOf(template: string): string | null {
  return /^\\(?:begin\{[a-zA-Z*]+\}|[a-zA-Z]+)/.exec(template)?.[0] ?? null;
}

let typedEntries: Map<string, MathEntry> | null = null;
let defaults: MathEntry[] | null = null;

/** How well an entry stands for its bare command: the command itself, then
 *  the command with only slots after it (`\mathbb{#{}}`, not `\mathbb{E}[#{}]`). */
function fit(name: string, template: string): number {
  if (template === name) return 0;
  return /^(?:\{\}|\[\])+$/.test(template.slice(name.length).replace(/[#$]\{[^{}]*\}/g, "")) ? 1 : 2;
}

/** Entries by leading command, the best fit for each (the first of a tie). */
function byCommand(entries: MathEntry[]): Map<string, MathEntry> {
  const out = new Map<string, MathEntry>();
  for (const entry of entries) {
    const name = commandOf(entry.template);
    const prev = name && out.get(name);
    if (name && (!prev || fit(name, entry.template) < fit(name, prev.template))) out.set(name, entry);
  }
  return out;
}

const tabEntries = () => MATH_TABS.flatMap((t) => t.entries);

/** The entry a typed `\command` counts as, or null for anything the palette
 *  and completion don't offer. */
export function entryForCommand(name: string): MathEntry | null {
  typedEntries ??= byCommand([...tabEntries(), ...MATH_COMMANDS]);
  return typedEntries.get(name) ?? null;
}

/** A command typed out and committed (TeX or the visual field). */
export function recordCommand(name: string, subject: number | null) {
  const entry = entryForCommand(name);
  if (entry) recordUse(entry, subject);
}

const cells = (e: MathEntry) => (e.wide ? 2 : 1);

/**
 * The Popular tab: the most used entries, then — while there is little
 * history — `POPULAR_DEFAULTS`, up to three rows of cells.
 */
export function popularEntries(): MathEntry[] {
  if (!defaults) {
    const all = [...tabEntries(), ...MATH_COMMANDS];
    defaults = POPULAR_DEFAULTS.flatMap((t) => all.find((e) => e.template === t) ?? []);
  }
  const out: MathEntry[] = [];
  const seen = new Set<string>();
  let used = 0;
  for (const entry of [...readUsage().sort(byUse), ...defaults]) {
    if (seen.has(entry.template) || used + cells(entry) > POPULAR_CELLS) continue;
    seen.add(entry.template);
    out.push(kept(entry));
    used += cells(entry);
    if (used === POPULAR_CELLS) break;
  }
  return out;
}

/** How many of a subject's last used entries are kept, to lead its picks. */
export const SUBJECT_RECENTS = 5;
/** How many entries the visual field's list offers before a letter is typed
 *  (after Space, or a bare `\`). */
export const FIELD_PICKS = 20;

/** The field's picks: the last few used in this subject, most recent first,
 *  then the Popular tab's. */
export function fieldPicks(subject: number | null): MathEntry[] {
  const out = subjectRecents(subject);
  const seen = new Set(out.map((e) => e.template));
  for (const entry of popularEntries()) {
    if (out.length === FIELD_PICKS) break;
    if (!seen.has(entry.template)) out.push(entry);
  }
  return out;
}
