import { renderToString } from "@/lib/maths";
import { COMMON_COMMANDS, MATH_COMMANDS, MATH_TABS, previewOf, type MathEntry } from "../../../tools/mathPalette";
import { commandOf } from "../../../tools/mathUsage";

/** The most options the list holds; it scrolls past what fits. */
const MAX_OPTIONS = 40;
/** The longest label shown before it is cut with an ellipsis. */
const LABEL_MAX = 18;

/** One palette entry the `\command` list offers. */
export interface CommandOption {
  /** The `\command` it starts with (`\sqrt`, `\begin{cases}`). */
  name: string;
  /** The palette template (`\sqrt[#{}]{#{}}`). */
  template: string;
  /** The palette entry, which counts as used once accepted. */
  entry: MathEntry;
  /** The template with its fields dropped, as the row reads it. */
  label: string;
  /** Rendered preview HTML, or null when it does not render. */
  preview: () => string | null;
}

let all: CommandOption[] | null = null;

const FIELD = /[#$]\{[^{}]*\}/g;

function labelOf(template: string): string {
  const label = template.replace(FIELD, "").replace(/\s+/g, " ").trim();
  return label.length > LABEL_MAX ? `${label.slice(0, LABEL_MAX - 1)}…` : label;
}

function optionOf(name: string, entry: MathEntry): CommandOption {
  let html: string | null | undefined;
  return {
    name,
    template: entry.template,
    entry,
    label: labelOf(entry.template),
    preview: () => {
      if (html !== undefined) return html;
      try {
        html = renderToString(previewOf(entry), { throwOnError: true });
      } catch {
        html = null;
      }
      return html;
    },
  };
}

/** How plainly a template stands for its command: the command alone, then
 *  with only slots after it (`\sqrt{#{}}`), then anything else. */
function plainness(c: CommandOption): number {
  if (c.template === c.name) return 0;
  return /^(?:\{\}|\[\])+$/.test(c.template.slice(c.name.length).replace(FIELD, "")) ? 1 : 2;
}

/** Every palette entry led by a `\command`, once per template: common
 *  commands first, then shorter names, a name's own templates plainest and
 *  then shortest first (`\sqrt{}` before `\sqrt[]{}`). */
function allOptions(): CommandOption[] {
  if (all) return all;
  const seen = new Set<string>();
  all = [];
  for (const entry of [...MATH_TABS.flatMap((t) => t.entries), ...MATH_COMMANDS]) {
    const name = commandOf(entry.template);
    if (!name || seen.has(entry.template)) continue;
    seen.add(entry.template);
    all.push(optionOf(name, entry));
  }
  const rank = (c: CommandOption) => (COMMON_COMMANDS.has(c.name) ? 0 : 1);
  all.sort(
    (a, b) =>
      rank(a) - rank(b) ||
      a.name.length - b.name.length ||
      a.name.localeCompare(b.name) ||
      plainness(a) - plainness(b) ||
      a.template.length - b.template.length,
  );
  return all;
}

/** What the list offers for the pending `name` (no backslash): the
 *  templates of the command typed exactly, then those of commands it
 *  starts. Nothing for an empty name: a bare `\` offers the picks. */
export function commandOptions(name: string): CommandOption[] {
  if (!name) return [];
  const typed = `\\${name}`;
  const exact = allOptions().filter((c) => c.name === typed);
  const longer = allOptions().filter((c) => c.name !== typed && c.name.startsWith(typed));
  return [...exact, ...longer].slice(0, MAX_OPTIONS);
}

/** Entries as options, in their order: the field's picks (`fieldPicks`). */
export function entryOptions(entries: readonly MathEntry[]): CommandOption[] {
  return entries.map((entry) => optionOf(commandOf(entry.template) ?? entry.template, entry));
}
