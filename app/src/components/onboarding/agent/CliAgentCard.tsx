import { CircleNotch } from "@phosphor-icons/react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import type { Provider } from "@/lib/harness";
import { AgentCard } from "./AgentCard";
import type { CliCardState } from "./cliCardState";

const CHIP: Record<Exclude<CliCardState, "ready" | "unknown">, string> = {
  checking: "Checking…",
  missing: "Not installed",
  installed: "Installed",
  out: "Signed out",
};

/** Claude Code, Codex or Antigravity: one account each, so the card is
 *  Install → Sign in → Ready. `installing` / `signingIn`: that run is live,
 *  with or without its dialog open. No `onInstall`: the agent installs from a
 *  terminal on this platform. */
export function CliAgentCard({
  provider,
  state,
  account,
  installing,
  signingIn,
  onInstall,
  onSignIn,
}: {
  provider: Provider;
  state: CliCardState;
  account: string | null;
  installing: boolean;
  signingIn: boolean;
  onInstall: (() => void) | null;
  onSignIn: () => void;
}) {
  const chip =
    state === "ready" ? (
      <>
        <Badge variant="success">Ready</Badge>
        {account && <span className="min-w-0 truncate text-xs text-muted-foreground">{account}</span>}
      </>
    ) : state === "unknown" ? null : (
      <Badge variant="secondary">{CHIP[state]}</Badge>
    );

  const action =
    state === "missing" && !onInstall ? (
      <span className="text-xs text-muted-foreground">Install it in a terminal</span>
    ) : state === "missing" ? (
      <Button variant="outline" size="xs" onClick={onInstall ?? undefined}>
        {installing && <CircleNotch className="animate-spin" />}
        {installing ? "Installing…" : "Install"}
      </Button>
    ) : state === "out" ? (
      <Button variant="outline" size="xs" onClick={onSignIn}>
        {signingIn && <CircleNotch className="animate-spin" />}
        {signingIn ? "Signing in…" : "Sign in"}
      </Button>
    ) : null;

  return <AgentCard provider={provider} chip={chip} actions={action} ready={state === "ready"} />;
}
