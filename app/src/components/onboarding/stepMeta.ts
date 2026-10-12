import { Books, GraduationCap, TerminalWindow, type Icon } from "@phosphor-icons/react";
import type { StepId } from "@/lib/onboarding/steps";

/** The steps between Welcome and Done, in order — the ones the progress
 *  rail, Welcome's preview and Done's summary show. */
export const SETUP_STEPS = ["canvas", "library", "agent"] as const satisfies readonly StepId[];

export type SetupStepId = (typeof SETUP_STEPS)[number];

export interface StepMeta {
  /** The short name on the rail and the summary tiles. */
  label: string;
  icon: Icon;
  /** One line on what the step sets up, for Welcome and Done. */
  blurb: string;
}

export const STEP_META: Record<SetupStepId, StepMeta> = {
  canvas: {
    label: "Canvas",
    icon: GraduationCap,
    blurb: "Sync your subjects, Ed threads and lecture recordings.",
  },
  library: {
    label: "Library",
    icon: Books,
    blurb: "Turn PDFs into text and let the agent search their pages.",
  },
  agent: {
    label: "Agent",
    icon: TerminalWindow,
    blurb: "Pick the coding agent that answers from your library.",
  },
};

export function isSetupStep(id: StepId): id is SetupStepId {
  return (SETUP_STEPS as readonly StepId[]).includes(id);
}
