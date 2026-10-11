import { useCallback, useEffect, useState } from "react";
import { ArrowCircleUp, ArrowsClockwise, CircleNotch } from "@phosphor-icons/react";
import {
  harnessAntigravityRevoke,
  harnessAntigravityRules,
  splitRule,
  type Provider,
} from "@/lib/harness";
import { useBridgeHealth } from "@/hooks/agents/useBridgeHealth";
import { signInAccount, signInState, useSignInStatus } from "@/hooks/agents/useSignInStatus";
import { SignInDialog, useSignIn } from "@/components/harness/SignInDialog";
import { Button } from "@/components/ui/button";
import { TerminalLine } from "@/components/markdown/MdComponents";
import { ProviderMark } from "@/components/icons/ProviderMark";
import {
  InstallAgentDialog,
  useAgentInstall,
} from "@/components/settings/agents/InstallAgentDialog";
import { UpdateOutputDialog, useAgentUpdates } from "@/components/settings/agents/UpdateAgents";
import { Section } from "@/components/settings/shared/section";

/**
 * The CLI agents behind Chat: where each binary is, its version, and whether
 * it is signed in. Health (`useBridgeHealth`) and sign-in (`useSignInStatus`)
 * are separate shared reads; *Recheck* is the only caller that drops Rust's
 * cached answers for both.
 *
 * A missing CLI offers *Install* (`InstallAgentDialog`). The run is held here,
 * not in the dialog, so closing it mid-install keeps the output and the
 * recheck its finish fires. opencode's sign-in is per provider, so its row
 * shows none (`signedIn: null`); the opencode page is the answer.
 *
 * An installed CLI with a newer published version offers *Update*
 * (`useAgentUpdates`); updates queue and run one at a time, and a failed one
 * keeps its command and output behind *Output*.
 */
function CliAgentsSection() {
  const { health, recheck, checking } = useBridgeHealth();
  const { statuses, recheck: recheckSignIn, checking: checkingSignIn } = useSignInStatus();
  const updates = useAgentUpdates();
  const recheckUpdates = updates.recheck;
  const recheckAll = useCallback(() => {
    recheckSignIn();
    // Versions first, so the update check compares the fresh ones.
    void recheck().then(recheckUpdates);
  }, [recheck, recheckSignIn, recheckUpdates]);
  const rechecking = checking || checkingSignIn || updates.checking;

  const install = useAgentInstall(recheck);
  const signIn = useSignIn(recheckSignIn);
  /** Which row's dialog is open; the run outlives the dialog. */
  const [openFor, setOpenFor] = useState<{ provider: Provider; label: string } | null>(null);
  const [signInFor, setSignInFor] = useState<Provider | null>(null);
  const [outputFor, setOutputFor] = useState<{ provider: Provider; label: string } | null>(null);
  const outputRun = outputFor ? updates.runs[outputFor.provider] : undefined;

  /** Available and not yet queued or running — what *Update all* enqueues. */
  const pending = (updates.updates ?? [])
    .filter((u) => u.available && updates.stateOf(u.provider) === null)
    .map((u) => u.provider);

  return (
    <Section
      title="CLI agents"
      description="Chat runs Claude Code, Codex, opencode or Antigravity from your own machine, signed in as you — no API key, no per-token billing."
    >
      <div className="divide-y divide-border-subtle">
        {(health ?? []).map((h) => {
          // `unknown` (not probed yet, or opencode) draws nothing.
          const state = h.path ? signInState(statuses, h.provider) : "unknown";
          const account = signInAccount(statuses, h.provider);
          const update = h.path ? updates.updates?.find((u) => u.provider === h.provider) : undefined;
          const run = updates.runs[h.provider];
          const updating = updates.stateOf(h.provider);
          const liveLines = run && !run.result && !run.error ? run.lines : undefined;
          const failed = !!run && (run.error !== null || run.result?.ok === false);
          return (
            <div key={h.provider} className="flex items-start justify-between gap-4 py-2.5">
              <div className="min-w-0">
                <div className="flex items-center gap-2 text-[13px] text-foreground">
                  <ProviderMark provider={h.provider} className="size-4 shrink-0" />
                  {h.label}
                </div>
                <div className="mt-0.5 truncate text-xs text-muted-foreground">
                  {h.path ?? (
                    <>
                      Not found. Install it, or set <span className="text-foreground">{h.overrideEnv}</span> to the binary.
                    </>
                  )}
                </div>
                {state !== "unknown" && (
                  <div className="mt-0.5 text-xs text-muted-foreground">
                    {state === "in" ? (account ? `Signed in — ${account}` : "Signed in") : "Signed out"}
                  </div>
                )}
                {h.error && h.path && <div className="mt-0.5 text-xs text-destructive">{h.error}</div>}
                {liveLines && <TerminalLine lines={liveLines} className="mt-1.5 max-w-full" />}
                {failed && (
                  <div className="mt-0.5 flex min-w-0 items-center gap-2 text-xs">
                    <span className="truncate text-destructive">
                      {run.error ?? `Update ${run.result?.status ?? "failed"}`}
                    </span>
                    <Button
                      variant="ghost"
                      size="xs"
                      className="shrink-0"
                      onClick={() => setOutputFor({ provider: h.provider, label: h.label })}
                    >
                      Output
                    </Button>
                  </div>
                )}
              </div>
              <div className="flex shrink-0 items-center gap-2">
                {update?.available && (
                  <Button
                    variant="outline"
                    size="xs"
                    disabled={updating === "queued"}
                    onClick={() =>
                      updating === "running"
                        ? setOutputFor({ provider: h.provider, label: h.label })
                        : updates.enqueue([h.provider])
                    }
                  >
                    {updating === "running" ? (
                      <>
                        <CircleNotch size={12} className="animate-spin" />
                        Updating…
                      </>
                    ) : updating === "queued" ? (
                      "Queued"
                    ) : (
                      `Update to v${update.latest}`
                    )}
                  </Button>
                )}
                <span className="text-xs tabular-nums text-muted-foreground">
                  {h.version ? `v${h.version}` : h.path ? "—" : "missing"}
                </span>
                {state === "out" && (
                  <Button variant="outline" size="xs" onClick={() => setSignInFor(h.provider)}>
                    {signIn.run?.provider === h.provider && !signIn.run.result ? (
                      <>
                        <CircleNotch size={12} className="animate-spin" />
                        Signing in…
                      </>
                    ) : (
                      "Sign in"
                    )}
                  </Button>
                )}
                {!h.path && (
                  <Button
                    variant="outline"
                    size="xs"
                    onClick={() => setOpenFor({ provider: h.provider, label: h.label })}
                  >
                    {install.run?.provider === h.provider && !install.run.result ? (
                      <>
                        <CircleNotch size={12} className="animate-spin" />
                        Installing…
                      </>
                    ) : (
                      "Install"
                    )}
                  </Button>
                )}
              </div>
            </div>
          );
        })}
        {health === null && (
          <div className="py-2.5 text-xs text-muted-foreground">Checking…</div>
        )}
      </div>
      <div className="mt-2 flex items-center gap-1">
        <Button variant="ghost" size="xs" onClick={recheckAll} disabled={rechecking}>
          {rechecking ? <CircleNotch size={12} className="animate-spin" /> : <ArrowsClockwise size={12} />}
          Recheck
        </Button>
        {pending.length > 0 && (
          <Button variant="ghost" size="xs" onClick={() => updates.enqueue(pending)}>
            <ArrowCircleUp size={12} />
            {pending.length > 1 ? `Update all (${pending.length})` : "Update all"}
          </Button>
        )}
      </div>
      {outputFor && outputRun && (
        <UpdateOutputDialog label={outputFor.label} run={outputRun} onClose={() => setOutputFor(null)} />
      )}
      {openFor && (
        <InstallAgentDialog
          provider={openFor.provider}
          label={openFor.label}
          run={install.run?.provider === openFor.provider ? install.run : null}
          onStart={(route) => install.start(openFor.provider, route)}
          onClose={() => {
            // A finished run is cleared with the dialog; a running one resumes
            // on reopen.
            if (install.run?.result) install.clear();
            setOpenFor(null);
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
            // Same rule as the install above.
            if (signIn.run?.result) signIn.clear();
            setSignInFor(null);
          }}
        />
      )}
    </Section>
  );
}

/** An approval in the chat allow button's words, not `agy`'s syntax. */
function ruleWords(rule: string): string {
  const r = splitRule(rule);
  if (!r) return rule;
  switch (r.action) {
    case "command":
      return `Run ${r.value}`;
    case "write_file":
      return `Write in ${r.value}`;
    case "read_file":
      return `Read ${r.value}`;
    case "read_url":
      return `Read pages on ${r.value}`;
  }
}

/**
 * What the student has let Antigravity do (from `PermissionCard`'s allow
 * button), with a way to revoke. Hidden until there is something to list;
 * once shown it stays for the visit so removing the last one doesn't jump the
 * page.
 */
function AntigravityApprovalsSection() {
  const [rules, setRules] = useState<string[] | null>(null);
  const [shown, setShown] = useState(false);
  const [removing, setRemoving] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    harnessAntigravityRules()
      .then((r) => {
        setRules(r);
        if (r.length) setShown(true);
      })
      .catch(() => setRules([]));
  }, []);

  const revoke = async (rule: string) => {
    setRemoving(rule);
    setError(null);
    try {
      setRules(await harnessAntigravityRevoke(rule));
    } catch (e) {
      setError(String(e));
    } finally {
      setRemoving(null);
    }
  };

  if (!shown || !rules) return null;
  return (
    <Section
      title="Antigravity approvals"
      description="What you let Antigravity do when it stopped to ask. They are kept in agy's own global settings, so agy in your terminal follows them too."
    >
      <div className="divide-y divide-border-subtle">
        {rules.map((rule) => (
          <div key={rule} className="flex items-center justify-between gap-4 py-2.5">
            <div className="min-w-0 truncate text-[13px] text-foreground" title={rule}>
              {ruleWords(rule)}
            </div>
            <Button
              variant="outline"
              size="xs"
              className="shrink-0"
              disabled={removing !== null}
              onClick={() => void revoke(rule)}
            >
              {removing === rule && <CircleNotch size={12} className="animate-spin" />}
              Remove
            </Button>
          </div>
        ))}
        {rules.length === 0 && (
          <div className="py-2.5 text-xs text-muted-foreground">No approvals.</div>
        )}
      </div>
      {error && <p className="mt-2 break-words text-xs text-destructive">{error}</p>}
    </Section>
  );
}

export default function SettingsAgentsPage() {
  return (
    <div className="flex flex-col gap-8">
      <CliAgentsSection />
      <AntigravityApprovalsSection />
    </div>
  );
}
