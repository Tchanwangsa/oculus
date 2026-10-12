import type { ComponentLevel } from "@/lib/studio/components";

/**
 * The message "Ask for a change" sends to the agent: a visible first line
 * naming the component, its level and its subject, then the request, then
 * how the oculus-studio skill is to make the change. The wording is here and
 * nowhere else.
 */
export function changeRequestText(c: {
  name: string;
  level: ComponentLevel;
  /** The subject's course code; only a subject-level component has one that matters. */
  subject: string | null;
  request: string;
}): string {
  const scope = c.level === "subject" && c.subject ? `subject level, ${c.subject}` : "global level";
  const library = c.level === "subject" && c.subject ? `the ${c.subject} library` : "the global library";
  const scratch =
    c.level === "subject" && c.subject
      ? `Make a scratch video with \`oculus studio new\` (subject ${c.subject})`
      : "Make a scratch video with `oculus studio new`";
  return [
    `Change the ${c.name} component (${scope}): ${c.request.trim()}`,
    "",
    `Use the oculus-studio skill. ${scratch}, put ${c.name} in its components/ folder and change it there, check every example with \`oculus studio gallery\`, then publish it to ${library} with \`oculus studio components promote <DIR> ${c.name} --to ${c.level}\`. Change only ${c.name}; keep its props and examples working.`,
  ].join("\n");
}
