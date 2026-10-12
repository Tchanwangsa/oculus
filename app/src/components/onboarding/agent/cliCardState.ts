import type { BridgeHealth, Provider, SignInStatus } from "@/lib/harness";
import { providerHealth } from "@/hooks/agents/useBridgeHealth";
import { signInState } from "@/hooks/agents/useSignInStatus";

/** What a CLI's card shows. `installed`: on this machine, but its sign-in
 *  check couldn't answer. `unknown`: the install check itself failed. */
export type CliCardState = "checking" | "unknown" | "missing" | "installed" | "out" | "ready";

export function cliCardState(
  health: BridgeHealth[] | null,
  statuses: SignInStatus[] | null,
  provider: Provider,
): CliCardState {
  if (health === null) return "checking";
  const installed = providerHealth(health, provider);
  if (installed === "unknown") return "unknown";
  if (installed === "missing") return "missing";
  if (statuses === null) return "checking";
  switch (signInState(statuses, provider)) {
    case "in":
      return "ready";
    case "out":
      return "out";
    case "unknown":
      return "installed";
  }
}
