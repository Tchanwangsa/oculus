import { useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getOnboarding, setOnboarding } from "@/lib/db/onboarding";
import { PROVIDER_IDS } from "@/lib/harness/providers";
import { gateFromRow, isExistingSetup } from "@/lib/onboarding/gate";
import { loadBridgeHealth, providerHealth } from "@/hooks/agents/useBridgeHealth";
import { loadSignInStatus, signInState } from "@/hooks/agents/useSignInStatus";
import { useOnboardingStore } from "@/stores/shell/onboardingStore";

/** Canvas first: without it the agents don't matter, and the sign-in check
 *  spawns every CLI (agy's takes seconds). `get_auth_status`, not `useAuth`,
 *  whose first answer is "disconnected" before it has asked. */
async function looksSetUp(): Promise<boolean> {
  const canvas = await invoke<boolean>("get_auth_status").catch(() => false);
  if (!canvas) return false;
  const [health, signIn] = await Promise.all([loadBridgeHealth(), loadSignInStatus()]);
  return isExistingSetup(
    canvas,
    PROVIDER_IDS.map((provider) => ({
      provider,
      installed: providerHealth(health, provider) === "installed",
      signIn: signInState(signIn, provider),
    })),
  );
}

async function decide(): Promise<boolean> {
  const gate = gateFromRow(await getOnboarding());
  if (gate !== "probe") return gate === "onboarding";
  if (!(await looksSetUp())) return true;
  await setOnboarding({ completedAt: new Date().toISOString(), skipped: [] });
  return false;
}

/** Once per window load, shared by StrictMode's double mount. */
let decided: Promise<void> | null = null;

/** The launch gate (docs/onboarding.md): whether onboarding replaces the
 *  shell, null while deciding. A gate that fails shows the app, which
 *  surfaces its own database errors. */
export function useOnboardingGate(): boolean | null {
  const open = useOnboardingStore((s) => s.open);
  useEffect(() => {
    decided ??= decide()
      .catch((e) => {
        console.error("onboarding gate failed", e);
        return false;
      })
      .then((open) => useOnboardingStore.setState({ open }));
  }, []);
  return open;
}
