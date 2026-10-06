import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { CheckCircle, Copy, CircleNotch, Warning } from "@phosphor-icons/react";

import type { Provider } from "@/lib/harness";
import { copyText } from "@/lib/utils";
import { useTauriEvent } from "@/hooks/useEvents";
import { Button } from "@/components/ui/button";
import { AnsiText, CodeText } from "@/components/markdown/MdComponents";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";

/** The tool a route needs, and the route's id: Rust looks the command up from
 *  this alone, so the webview never names a command. */
export type InstallManager = "curl" | "brew" | "npm" | "bun";

export interface InstallRoute {
  manager: InstallManager;
  label: string;
  command: string;
  landsIn: string;
  /** Whether the tool it needs is on this machine. */
  available: boolean;
}

export interface InstallOffer {
  provider: Provider;
  label: string;
  routes: InstallRoute[];
  runnable: boolean;
}

/** One line of a running install, or the last event of the run. */
interface InstallLine {
  provider: Provider;
  line: string | null;
  done: boolean;
  ok: boolean | null;
  status: string | null;
}

const INSTALL_EVENT = "harness-install";

export interface AgentInstallRun {
  provider: Provider;
  route: InstallRoute;
  lines: string[];
  result: { ok: boolean; status: string } | null;
  error: string | null;
}

/**
 * The install run, held by the section rather than the dialog so closing the
 * dialog mid-install keeps the output and still fires the recheck.
 * `onFinished` (Settings' Recheck) runs however the command ended — a non-zero
 * exit can still leave a working binary.
 */
export function useAgentInstall(onFinished: () => void): {
  run: AgentInstallRun | null;
  start: (provider: Provider, route: InstallRoute) => void;
  clear: () => void;
} {
  const [run, setRun] = useState<AgentInstallRun | null>(null);

  // One listener for the section's life: attached per click it would miss the
  // first lines, since `listen` resolves after the child starts talking.
  useTauriEvent<InstallLine>(INSTALL_EVENT, (e) => {
    const ev = e.payload;
    setRun((prev) => {
      if (!prev || prev.provider !== ev.provider) return prev;
      if (ev.done) return { ...prev, result: { ok: ev.ok ?? false, status: ev.status ?? "finished" } };
      return ev.line === null ? prev : { ...prev, lines: [...prev.lines, ev.line] };
    });
    if (ev.done) onFinished();
  });

  const start = useCallback((provider: Provider, route: InstallRoute) => {
    setRun({ provider, route, lines: [], result: null, error: null });
    invoke("harness_install_run", { provider, manager: route.manager }).catch((e) =>
      setRun((prev) =>
        prev && prev.provider === provider ? { ...prev, error: String(e) } : prev,
      ),
    );
  }, []);

  const clear = useCallback(() => setRun(null), []);

  return { run, start, clear };
}

/**
 * Installs a missing CLI agent without owning the user's package manager.
 * Rust reports which of `brew`/`npm`/`bun`/`curl` a login shell can see (a
 * Dock-launched app lacks the profile PATH); every route shows its literal
 * command with Copy, and *Install* runs exactly that via `$SHELL -lc`. Nothing
 * needing `sudo` is offered — a GUI app cannot prompt for it.
 *
 * The output stream is listened to in `useAgentInstall`, not a store: it is a
 * foreground job with no meaning once Settings is left.
 */
export function InstallAgentDialog({
  provider,
  label,
  run,
  onStart,
  onClose,
}: {
  provider: Provider;
  label: string;
  run: AgentInstallRun | null;
  onStart: (route: InstallRoute) => void;
  onClose: () => void;
}) {
  const [offer, setOffer] = useState<InstallOffer | null>(null);
  const [failed, setFailed] = useState<string | null>(null);
  const [copied, setCopied] = useState<InstallManager | null>(null);

  useEffect(() => {
    let live = true;
    invoke<InstallOffer>("harness_install_offer", { provider })
      .then((o) => live && setOffer(o))
      .catch((e) => live && setFailed(String(e)));
    return () => {
      live = false;
    };
  }, [provider]);

  const log = useRef<HTMLDivElement>(null);
  useEffect(() => {
    // Follow the tail.
    const el = log.current;
    if (el) el.scrollTop = el.scrollHeight;
  }, [run?.lines, run?.result]);

  const copy = async (route: InstallRoute) => {
    if (await copyText(route.command)) {
      setCopied(route.manager);
      window.setTimeout(() => setCopied((c) => (c === route.manager ? null : c)), 1200);
    }
  };

  // With something runnable, list only runnable routes; otherwise list every
  // command for copying.
  const listed = offer ? (offer.runnable ? offer.routes.filter((r) => r.available) : offer.routes) : [];

  const description = run
    ? run.result
      ? run.result.ok
        ? `${label} is installed. Its row has been rechecked.`
        : `The command ${run.result.status}. The output is below.`
      : "Running. This usually takes a minute, and the output is live."
    : offer && !offer.runnable
      ? "This machine has no Homebrew, node or bun that Oculus can drive, and no curl to fetch an installer with. Copy a command and run it wherever you have a shell."
      : `Oculus runs the command you pick, exactly as it is written here. Nothing here needs your password.`;

  return (
    <Dialog open onOpenChange={(open) => !open && onClose()}>
      <DialogContent className="sm:max-w-lg">
        <DialogHeader>
          <DialogTitle>Install {label}</DialogTitle>
          <DialogDescription>{description}</DialogDescription>
        </DialogHeader>

        {!offer && !failed && !run && (
          <div className="flex items-center gap-2 py-2 text-xs text-muted-foreground">
            <CircleNotch size={12} className="animate-spin" />
            <span>Looking at what this machine has…</span>
          </div>
        )}

        {offer && !run && (
          <div className="flex flex-col gap-2">
            {listed.map((route) => (
              <div key={route.manager} className="rounded-lg border border-border-subtle p-2.5">
                <div className="flex items-center justify-between gap-3">
                  <div className="min-w-0 truncate text-xs text-muted-foreground">
                    {route.label}
                    <span className="mx-1.5 text-border">·</span>
                    installs to {route.landsIn}
                  </div>
                  <div className="flex shrink-0 items-center gap-1">
                    <Button variant="ghost" size="xs" onClick={() => void copy(route)}>
                      {copied === route.manager ? <CheckCircle size={12} /> : <Copy size={12} />}
                      {copied === route.manager ? "Copied" : "Copy"}
                    </Button>
                    {route.available && (
                      <Button size="xs" onClick={() => onStart(route)}>
                        Install
                      </Button>
                    )}
                  </div>
                </div>
                <CodeText className="mt-2 text-muted-foreground">{route.command}</CodeText>
              </div>
            ))}
          </div>
        )}

        {run && (
          <div className="flex flex-col gap-2">
            <CodeText className="text-muted-foreground">{run.route.command}</CodeText>
            <div
              ref={log}
              className="max-h-64 overflow-y-auto rounded-lg border border-border-subtle bg-surface p-2.5"
            >
              {run.lines.length === 0 && !run.result ? (
                <div className="flex items-center gap-2 text-xs text-muted-foreground">
                  <CircleNotch size={12} className="animate-spin" />
                  <span>Starting…</span>
                </div>
              ) : (
                <AnsiText lines={run.lines} />
              )}
            </div>
            {run.result && (
              <div
                className={`flex items-center gap-2 text-xs ${run.result.ok ? "text-success" : "text-destructive"}`}
              >
                {run.result.ok ? <CheckCircle size={12} /> : <Warning size={12} />}
                <span>
                  {run.result.ok
                    ? "Installed."
                    : "Copy the command and run it in a terminal to see what it wanted."}
                </span>
              </div>
            )}
            {run.error && <p className="text-xs text-destructive">{run.error}</p>}
          </div>
        )}

        {failed && <p className="text-xs text-destructive">{failed}</p>}

        <DialogFooter>
          <Button variant="outline" size="sm" onClick={onClose}>
            {run && !run.result ? "Hide" : "Close"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
