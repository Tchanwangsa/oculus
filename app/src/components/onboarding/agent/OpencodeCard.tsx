import { CircleNotch } from "@phosphor-icons/react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import type { OpencodeProvider } from "@/lib/harness/opencodeAuth";
import { AgentCard } from "./AgentCard";
import type { CliCardState } from "./cliCardState";

/**
 * What `OpencodeConnectDialog` needs to take an OpenRouter key, written out so
 * the card never lists providers: that read starts `opencode serve`, which
 * only the Connect click may do (docs/harness.md). OpenRouter declares no
 * method in `/provider/auth`, so opencode's answer would be this same plain
 * API key at index 0, and Rust re-reads the method by index before saving.
 */
export const OPENROUTER: OpencodeProvider = {
  id: "openrouter",
  name: "OpenRouter",
  source: "",
  env: ["OPENROUTER_API_KEY"],
  modelCount: 0,
  connected: false,
  methods: [{ index: 0, kind: "api", label: "API key", prompts: [] }],
};

/** opencode holds a credential per provider, so there is no one sign-in to
 *  show: the card offers Install, then Ready once installed (the same rule as
 *  the launch gate), with Connect OpenRouter for an opencode that has no
 *  provider yet. `connected`: a key was saved during this setup. */
export function OpencodeCard({
  state,
  connected,
  installing,
  onInstall,
  onConnect,
}: {
  state: CliCardState;
  connected: boolean;
  installing: boolean;
  onInstall: () => void;
  onConnect: () => void;
}) {
  const installed = state !== "checking" && state !== "unknown" && state !== "missing";

  const chip = connected ? (
    <Badge variant="success">Connected</Badge>
  ) : state === "checking" ? (
    <Badge variant="secondary">Checking…</Badge>
  ) : state === "missing" ? (
    <Badge variant="secondary">Not installed</Badge>
  ) : installed ? (
    <Badge variant="success">Ready</Badge>
  ) : null;

  const action =
    state === "missing" ? (
      <Button variant="outline" size="xs" onClick={onInstall}>
        {installing && <CircleNotch className="animate-spin" />}
        {installing ? "Installing…" : "Install"}
      </Button>
    ) : installed && !connected ? (
      <Button variant="outline" size="xs" onClick={onConnect}>
        Connect OpenRouter
      </Button>
    ) : null;

  return (
    <AgentCard
      provider="opencode"
      chip={chip}
      actions={action}
      hint={
        installed && !connected
          ? "Only needed if opencode has no provider set up yet."
          : undefined
      }
      ready={connected || installed}
    />
  );
}
