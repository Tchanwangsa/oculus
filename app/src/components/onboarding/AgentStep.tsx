import { useCallback, useState } from "react";
import { ArrowsClockwise, CircleNotch } from "@phosphor-icons/react";
import { SignInDialog, useSignIn } from "@/components/harness/SignInDialog";
import {
  InstallAgentDialog,
  useAgentInstall,
  useAgentInstallOffers,
} from "@/components/settings/agents/InstallAgentDialog";
import { OpencodeConnectDialog } from "@/components/settings/agents/OpencodeConnectDialog";
import { Button } from "@/components/ui/button";
import { providerHealth, useBridgeHealth } from "@/hooks/agents/useBridgeHealth";
import { signInAccount, signInState, useSignInStatus } from "@/hooks/agents/useSignInStatus";
import { providerAvailability, providerLabel, type Provider } from "@/lib/harness";
import { rememberProviders } from "@/lib/harness/opencodeAuth";
import { AGENT_ORDER, orderAgents } from "@/lib/onboarding/agentOptions";
import { isAgentReady } from "@/lib/onboarding/gate";
import { CliAgentCard } from "./agent/CliAgentCard";
import { cliCardState } from "./agent/cliCardState";
import { OPENROUTER, OpencodeCard } from "./agent/OpencodeCard";
import { SyncProgressLine } from "./library/SyncProgressLine";
import { STEP_META } from "./stepMeta";
import { StepFrame } from "./StepFrame";
import type { StepProps } from "./types";

/**
 * Every CLI agent as an equal card, the ready ones first, each opening the
 * same Install and Sign in dialogs as Settings → Agents. The install and
 * sign-in runs live here, so closing a dialog mid-run keeps it going. opencode
 * offers Connect OpenRouter instead of a sign-in; that click is the only thing
 * here that starts `opencode serve`. Next waits for one agent to be ready,
 * by the launch gate's rule (an installed opencode counts).
 */
export function AgentStep({ onNext, onSkip }: StepProps) {
  const { health, recheck: recheckHealth, checking } = useBridgeHealth();
  const { statuses, recheck: recheckSignIn, checking: checkingSignIn } = useSignInStatus();
  const recheck = useCallback(() => {
    recheckSignIn();
    void recheckHealth();
  }, [recheckHealth, recheckSignIn]);

  const install = useAgentInstall(recheck);
  const offers = useAgentInstallOffers(health);
  const signIn = useSignIn(recheck);
  const [installFor, setInstallFor] = useState<Provider | null>(null);
  const [signInFor, setSignInFor] = useState<Provider | null>(null);
  const [connecting, setConnecting] = useState(false);
  /** A key was saved this session; opencode is ready once installed, so this
   *  only changes its chip. */
  const [opencodeConnected, setOpencodeConnected] = useState(false);

  const stateOf = (p: Provider) => cliCardState(health, statuses, p);
  const isReady = (p: Provider) =>
    (p === "opencode" && opencodeConnected) ||
    isAgentReady({
      provider: p,
      installed: providerHealth(health, p) === "installed",
      signIn: signInState(statuses, p),
    });

  // Sorted once, when both checks first answer, so a card never moves under
  // the pointer after a sign-in. Set during render to skip a painted frame in
  // the neutral order.
  const [order, setOrder] = useState<Provider[] | null>(null);
  if (order === null && health !== null && statuses !== null) setOrder(orderAgents(isReady));

  // Windows has no native Claude Code (docs/harness.md).
  const offered = (order ?? AGENT_ORDER).filter((p) => providerAvailability(p).supported);
  const anyReady = offered.some(isReady);
  const rechecking = checking || checkingSignIn;
  const liveInstall = (p: Provider) => install.run?.provider === p && !install.run.result;

  return (
    <StepFrame
      title="Choose an agent"
      description="Oculus works with the coding agent you already use. Pick one or more."
      icon={STEP_META.agent.icon}
      onNext={onNext}
      nextDisabled={!anyReady}
      onSkip={onSkip}
    >
      <SyncProgressLine />
      <div className="flex flex-col gap-3">
        <div className="flex flex-col gap-2.5">
          {offered.map((p) =>
            p === "opencode" ? (
              <OpencodeCard
                key={p}
                state={stateOf(p)}
                connected={opencodeConnected}
                installing={liveInstall(p)}
                onInstall={() => setInstallFor(p)}
                onConnect={() => setConnecting(true)}
              />
            ) : (
              <CliAgentCard
                key={p}
                provider={p}
                state={stateOf(p)}
                account={signInAccount(statuses, p)}
                installing={liveInstall(p)}
                signingIn={signIn.run?.provider === p && !signIn.run.result}
                onInstall={
                  // `undefined` is an offer still loading; `null` has no route.
                  providerAvailability(p).install && offers[p] !== null
                    ? () => offers[p] && setInstallFor(p)
                    : null
                }
                onSignIn={() => setSignInFor(p)}
              />
            ),
          )}
        </div>
        <div>
          <Button
            variant="ghost"
            size="xs"
            className="text-muted-foreground"
            onClick={recheck}
            disabled={rechecking}
          >
            {rechecking ? <CircleNotch className="animate-spin" /> : <ArrowsClockwise />}
            Check again
          </Button>
        </div>
      </div>

      {installFor && offers[installFor] && (
        <InstallAgentDialog
          offer={offers[installFor]}
          label={providerLabel(installFor)}
          run={install.run?.provider === installFor ? install.run : null}
          onStart={(route) => install.start(installFor, route)}
          onClose={() => {
            // A finished run is cleared with the dialog; a running one resumes
            // on reopen.
            if (install.run?.result) install.clear();
            setInstallFor(null);
          }}
        />
      )}
      {signInFor && (
        <SignInDialog
          provider={signInFor}
          run={signIn.run?.provider === signInFor ? signIn.run : null}
          onStart={() => signIn.start(signInFor)}
          onCode={(code) => signIn.submitCode(code)}
          onCancel={() => signIn.cancel()}
          onClose={() => {
            if (signIn.run?.result) signIn.clear();
            setSignInFor(null);
          }}
        />
      )}
      {connecting && (
        <OpencodeConnectDialog
          provider={OPENROUTER}
          onClose={() => setConnecting(false)}
          onDone={(list) => {
            // Settings → opencode reads this instead of asking the server again.
            rememberProviders(list);
            setOpencodeConnected(true);
            setConnecting(false);
            recheck();
          }}
        />
      )}
    </StepFrame>
  );
}
