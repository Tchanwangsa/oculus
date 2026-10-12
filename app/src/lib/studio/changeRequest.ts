import type { ComponentLevel } from "@/lib/studio/components";

/** What "Ask for a change" can change: a component or a library theme. */
export type LibraryItemKind = "component" | "theme";

/**
 * The message "Ask for a change" sends to the agent: a visible first line
 * naming the component or theme, its level and its subject, then the
 * request, then how the oculus-studio skill is to make the change. Both go
 * through a scratch video and `promote`, so the library keeps the old text
 * for Undo. The wording is here and nowhere else.
 */
export function changeRequestText(c: {
  kind: LibraryItemKind;
  name: string;
  level: ComponentLevel;
  /** The subject's course code; only a subject-level item has one that matters. */
  subject: string | null;
  request: string;
}): string {
  const own = c.level === "subject" && c.subject ? c.subject : null;
  const scope = own ? `subject level, ${own}` : "global level";
  const library = own ? `the ${own} library` : "the global library";
  const scratch = own
    ? `Make a scratch video with \`oculus studio new\` (subject ${own})`
    : "Make a scratch video with `oculus studio new`";
  if (c.kind === "theme") {
    const file = own ? `../studio/subjects/${own}/themes/${c.name}.json` : `../studio/library/themes/${c.name}.json`;
    return [
      `Change the ${c.name} theme (${scope}): ${c.request.trim()}`,
      "",
      `Use the oculus-studio skill. The theme is \`${file}\` in ${library}. ${scratch}, copy that file to the video's \`themes/${c.name}.json\` and change it there (keep every section and token name), check it with \`oculus studio gallery <DIR> --theme themes/${c.name}.json\`, then write it back to ${library} with \`oculus studio themes promote <DIR> themes/${c.name}.json --to ${c.level}\`. Change only the ${c.name} theme.`,
    ].join("\n");
  }
  return [
    `Change the ${c.name} component (${scope}): ${c.request.trim()}`,
    "",
    `Use the oculus-studio skill. ${scratch}, put ${c.name} in its components/ folder and change it there, check every example with \`oculus studio gallery\`, then publish it to ${library} with \`oculus studio components promote <DIR> ${c.name} --to ${c.level}\`. Change only ${c.name}; keep its props and examples working.`,
  ].join("\n");
}
