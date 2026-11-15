import type { Capabilities } from "./capabilities";
import { providerLabel } from "./registry";
import type { PermissionMode } from "./types";

/** The composer's control for how a chat asks (docs/harness.md "Requests"). */

export const PERMISSION_MODES: PermissionMode[] = ["ask", "auto", "quiet"];

export const MODE_LABEL: Record<PermissionMode, string> = {
  ask: "Ask",
  auto: "Auto",
  quiet: "Don't ask",
};

const MODE_LINE: Record<PermissionMode, string> = {
  ask: "Asks you before the agent runs anything its sandbox blocks",
  auto: "The agent's own reviewer decides; you are not asked",
  quiet: "Anything the sandbox blocks is declined",
};

export interface ModeChoice {
  mode: PermissionMode;
  label: string;
  line: string;
}

/** What the control shows for an agent: a note when it cannot ask at all,
 *  and each mode with its one line. Auto says it asks as Ask where the
 *  agent has no reviewer. */
export function modeChoices(caps: Pick<Capabilities, "provider" | "approvals" | "autoReview">): {
  note: string | null;
  choices: ModeChoice[];
} {
  const who = providerLabel(caps.provider);
  const note = caps.approvals
    ? null
    : `${who} cannot ask yet: anything its sandbox blocks is declined, whichever you choose.`;
  const choices = PERMISSION_MODES.map((mode) => ({
    mode,
    label: MODE_LABEL[mode],
    line:
      mode === "auto" && caps.approvals && !caps.autoReview
        ? `${who} has no reviewer, so Auto asks you, as Ask does`
        : MODE_LINE[mode],
  }));
  return { note, choices };
}

/** The mode a chat runs under; Rust's default for a chat is `ask`. */
export const threadMode = (mode: PermissionMode | null | undefined): PermissionMode => mode ?? "ask";
