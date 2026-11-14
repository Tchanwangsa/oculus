import { insertRankedSearchResult, normalizeSearchQuery, scoreQueryMatch } from "@/lib/search/ranking";
import { scoreSkill, type Skill } from "./skills";

/** What a command does on pick. Each acts in the app, removes its `/word`
 *  and sends nothing. */
export type ComposerCommandId = "model";

export interface ComposerCommand {
  id: ComposerCommandId;
  /** Typed after the `/`. */
  name: string;
  description: string;
}

/** What the open thread's bridge can do, for commands only some bridges have. */
export type CommandCaps = Record<string, never>;

/** The commands Oculus implements, each with the bridge capability it needs.
 *  `/compact` joins as one entry gated on the bridge's `compact` flag. */
const COMMANDS: { command: ComposerCommand; available: (caps: CommandCaps) => boolean }[] = [
  {
    command: { id: "model", name: "model", description: "Switch response model for this thread" },
    available: () => true,
  },
];

/** The commands this composer may offer. */
export function composerCommands(caps: CommandCaps = {}): ComposerCommand[] {
  return COMMANDS.filter((c) => c.available(caps)).map((c) => c.command);
}

function scoreCommand(command: ComposerCommand, query: string): number | null {
  const scores = [
    scoreQueryMatch({
      value: command.name.toLowerCase(),
      query,
      exactBase: 0,
      prefixBase: 2,
      boundaryBase: 4,
      includesBase: 6,
      fuzzyBase: 100,
      boundaryMarkers: ["-", "_", "/"],
    }),
    scoreQueryMatch({
      value: command.description.toLowerCase(),
      query,
      exactBase: 20,
      prefixBase: 22,
      boundaryBase: 24,
      includesBase: 26,
    }),
  ].filter((score): score is number => score !== null);
  return scores.length ? Math.min(...scores) : null;
}

/** The `/` menu's two groups for a query, each best first; for an empty
 *  query every command and every skill in the order given. T3's
 *  `searchSlashCommandItems`, split into its groups. */
export function searchSlashItems(
  commands: readonly ComposerCommand[],
  skills: readonly Skill[],
  query: string,
): { commands: ComposerCommand[]; skills: Skill[] } {
  const q = normalizeSearchQuery(query, { trimLeadingPattern: /^\/+/ });
  if (!q) return { commands: [...commands], skills: [...skills] };
  const rankedCommands: { item: ComposerCommand; score: number; tieBreaker: string }[] = [];
  for (const command of commands) {
    const score = scoreCommand(command, q);
    if (score !== null) insertRankedSearchResult(rankedCommands, { item: command, score, tieBreaker: command.name }, Infinity);
  }
  const rankedSkills: { item: Skill; score: number; tieBreaker: string }[] = [];
  for (const skill of skills) {
    const score = scoreSkill(skill, q);
    if (score !== null) insertRankedSearchResult(rankedSkills, { item: skill, score, tieBreaker: skill.name.toLowerCase() }, Infinity);
  }
  return { commands: rankedCommands.map((r) => r.item), skills: rankedSkills.map((r) => r.item) };
}
