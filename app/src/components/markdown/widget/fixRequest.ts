/** What Ask to fix sends: the error and the whole fence, so the agent can
 *  resend a corrected widget in its reply. */
export function fixRequest(title: string, error: string, source: string, language: "widget" | "openui" = "widget"): string {
  const reason = error.trim().replace(/[.\s]+$/, "");
  return `The widget "${title}" failed: ${reason}. Fix it and resend the widget.\n\n\`\`\`${language}\n${source}\n\`\`\``;
}
