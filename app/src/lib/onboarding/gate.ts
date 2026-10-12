import type { OnboardingRow } from "@/lib/db/onboarding";
import type { Provider } from "@/lib/harness/providers";
import { providerAvailability } from "@/lib/harness/availability";

/** What launch does with the settings row alone. `probe`: the row is absent,
 *  so ask whether this is a setup from before onboarding existed. */
export type RowGate = "app" | "onboarding" | "probe";

export function gateFromRow(row: OnboardingRow | null): RowGate {
  if (!row) return "probe";
  return row.completedAt ? "app" : "onboarding";
}

/** One agent as the probe sees it: `useBridgeHealth` and `useSignInStatus`
 *  answers, flattened. */
export interface AgentProbe {
  provider: Provider;
  installed: boolean;
  signIn: "unknown" | "in" | "out";
}

/** Installed and signed in. opencode's sign-in can't be asked from outside
 *  its own page (always `unknown`), so installed is enough for it. */
export function isAgentReady(agent: AgentProbe, windows?: boolean): boolean {
  if (!agent.installed || !providerAvailability(agent.provider, windows).supported) return false;
  return agent.provider === "opencode" || agent.signIn === "in";
}

/** A setup that predates onboarding: Canvas signed in and one agent ready.
 *  It gets `completedAt` written silently and never sees onboarding. */
export function isExistingSetup(canvasAuthenticated: boolean, agents: AgentProbe[], windows?: boolean): boolean {
  return canvasAuthenticated && agents.some((agent) => isAgentReady(agent, windows));
}
