import { invoke } from "@tauri-apps/api/core";
import { insertRankedSearchResult, normalizeSearchQuery, scoreQueryMatch } from "@/lib/search/ranking";

/** Where a skill was found: the library's own skills (`agents/.claude/skills`,
 *  `agents/.agents/skills`), or the student's home folders (`~/.claude/skills`,
 *  `~/.agents/skills`). */
export type SkillSource = "oculus" | "personal";

/** One installed skill, as `harness_skills` reads it off its `SKILL.md`. */
export interface Skill {
  name: string;
  description: string;
  /** The `SKILL.md` it was read from, absolute. */
  path: string;
  source: SkillSource;
  /** False when its frontmatter says `user-invocable: false`. */
  userInvocable: boolean;
}

/** The badge a menu row shows for a source. */
export const SKILL_SOURCE_LABEL: Record<SkillSource, string> = { oculus: "Oculus", personal: "Personal" };

/** Every skill the composer can offer, Oculus's first, one per name. Scans
 *  four folders on disk; no network, no model. */
export async function harnessSkills(): Promise<Skill[]> {
  const found = await invoke<Skill[]>("harness_skills");
  return dedupeSkillsByName(found.filter((s) => s.userInvocable));
}

/** The first skill of each name, in the order given (Rust lists Oculus's first). */
export function dedupeSkillsByName(skills: readonly Skill[]): Skill[] {
  const seen = new Set<string>();
  return skills.filter((s) => {
    if (seen.has(s.name)) return false;
    seen.add(s.name);
    return true;
  });
}

/** A skill against a normalised query: name before description before
 *  source; null when nothing matches. T3's `scoreProviderSkill`, less the
 *  display name and short description our frontmatter does not carry. */
export function scoreSkill(skill: Skill, query: string): number | null {
  const scores = [
    scoreQueryMatch({
      value: skill.name.toLowerCase(),
      query,
      exactBase: 0,
      prefixBase: 2,
      boundaryBase: 4,
      includesBase: 6,
      fuzzyBase: 100,
      boundaryMarkers: ["-", "_", "/"],
    }),
    scoreQueryMatch({
      value: skill.description.toLowerCase(),
      query,
      exactBase: 30,
      prefixBase: 32,
      boundaryBase: 34,
      includesBase: 36,
    }),
    scoreQueryMatch({
      value: SKILL_SOURCE_LABEL[skill.source].toLowerCase(),
      query,
      exactBase: 40,
      prefixBase: 42,
      includesBase: 44,
    }),
  ].filter((score): score is number => score !== null);
  return scores.length ? Math.min(...scores) : null;
}

/** The skills matching `query` (a leading `/` ignored), best first; all of
 *  them, in catalogue order, for an empty query. */
export function searchSkills(skills: readonly Skill[], query: string, limit = Number.POSITIVE_INFINITY): Skill[] {
  const q = normalizeSearchQuery(query, { trimLeadingPattern: /^\/+/ });
  if (!q) return skills.slice(0, Number.isFinite(limit) ? limit : undefined);
  const ranked: { item: Skill; score: number; tieBreaker: string }[] = [];
  for (const skill of skills) {
    const score = scoreSkill(skill, q);
    if (score === null) continue;
    insertRankedSearchResult(ranked, { item: skill, score, tieBreaker: skill.name.toLowerCase() }, limit);
  }
  return ranked.map((r) => r.item);
}

/** A skill as the message spells it. */
export function skillText(name: string): string {
  return `/${name}`;
}

/** `/name` standing as a word of its own: at the start or after whitespace,
 *  and followed by whitespace or the end. Names are a skill folder's: letters,
 *  digits, `.`, `_`, `:` and `-`. */
const SKILL_TOKEN = /(^|\s)\/([A-Za-z0-9][A-Za-z0-9._:-]*)(?=\s|$)/g;

/** Text split around the `/name` words that name a skill in `known`; other
 *  slashes stay text. What turns a stored draft's `/name` back into a chip. */
export function splitSkillTokens(
  text: string,
  known: ReadonlySet<string>,
): ({ kind: "text"; text: string } | { kind: "skill"; name: string })[] {
  const out: ({ kind: "text"; text: string } | { kind: "skill"; name: string })[] = [];
  let at = 0;
  for (const m of text.matchAll(SKILL_TOKEN)) {
    const name = m[2];
    if (!known.has(name)) continue;
    const start = (m.index ?? 0) + m[1].length;
    if (start > at) out.push({ kind: "text", text: text.slice(at, start) });
    out.push({ kind: "skill", name });
    at = start + name.length + 1;
  }
  if (at < text.length) out.push({ kind: "text", text: text.slice(at) });
  return out;
}

/** The skills picked as chips in the message a composer is sending, until
 *  `harnessSend` takes them for that same text. One slot: a composer notes
 *  its chips just before handing the text on, as an open notebook rides
 *  `harnessSend` without every caller passing it. */
let picked: { text: string; names: string[] } | null = null;

/** Called by a composer as it sends `text` holding these skill chips. */
export function notePickedSkills(text: string, names: readonly string[]): void {
  picked = names.length ? { text, names: [...new Set(names)] } : null;
}

/** The skills picked for `text`, once; none for text a composer did not note,
 *  so a typed `/word` is only ever text. */
export function takePickedSkills(text: string): string[] {
  if (!picked || picked.text !== text) return [];
  const { names } = picked;
  picked = null;
  return names;
}
