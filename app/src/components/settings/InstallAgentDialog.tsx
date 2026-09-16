import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { CheckCircle, Copy, CircleNotch, Warning } from "@phosphor-icons/react";

import type { Provider } from "@/lib/harness";
import { copyText } from "@/lib/utils";
import { Button } from "@/components/ui/button";
import { CodeText } from "@/components/markdown/MdComponents";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";

/** The tool a route needs. It is also the route's id: one route per manager
 *  per provider, so Rust looks the command up from this alone — the webview
 *  never names a command, which is why an invoke cannot become a shell. */
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
 * The run itself, held by the **section** rather than by the dialog.
 *
 * Which looks like indirection and is the opposite: the dialog can be closed
 * mid-install, and if the stream and the recheck lived in it, closing it would
 * strand a running child — no output kept, and, worse, no recheck, which is
 * the step that turns "missing" into a version number. Held a level up, the
 * dialog is only ever a view of this, reopening it resumes the same run, and
 * the recheck fires whether or not anyone is looking.
 *
 * `onFinished` is Settings' *Recheck*, and it runs however the command ended:
 * a non-zero exit can still have left a working binary, and a recheck is far
 * cheaper than a row that lies.
 */
export function useAgentInstall(onFinished: () => void): {
  run: AgentInstallRun | null;
  start: (provider: Provider, route: InstallRoute) => void;
  clear: () => void;
} {
  const [run, setRun] = useState<AgentInstallRun | null>(null);

  const finished = useRef(onFinished);
  finished.current = onFinished;

  // One listener for the life of the section, not one per run: attached on
  // the click it would miss the first lines, since `listen` resolves a tick
  // after the child is already talking.
  useEffect(() => {
    const un = listen<InstallLine>(INSTALL_EVENT, (e) => {
      const ev = e.payload;
      setRun((prev) => {
        if (!prev || prev.provider !== ev.provider) return prev;
        if (ev.done) return { ...prev, result: { ok: ev.ok ?? false, status: ev.status ?? "finished" } };
        return ev.line === null ? prev : { ...prev, lines: [...prev.lines, ev.line] };
      });
      if (ev.done) finished.current();
    });
    return () => {
      void un.then((f) => f());
    };
  }, []);

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
 * Installing a missing CLI agent, from the row in Settings → AI that says it
 * is missing.
 *
 * The shape is the one that was agreed rather than the obvious one, and the
 * difference is the point: this does **not** own the student's package
 * manager. Homebrew may not be here and installing it needs `sudo`, which a
 * GUI app has no way to prompt for; and a `curl … | bash` fired by an app that
 * never showed the command is the wrong trust posture for the one app that
 * also holds a Canvas session. So:
 *
 * 1. Rust says which of `brew`, `npm`, `bun` and `curl` this machine has,
 *    found the way the CLIs themselves are found — a login shell, because a
 *    Dock-launched app has launchd's PATH and `brew` is not on it.
 * 2. Every route is shown as its **literal command**, with Copy. That is the
 *    path that always works — a locked-down machine, or a student who would
 *    rather run it in their own terminal — so it is there even beside the
 *    button that would run it.
 * 3. *Install* runs exactly the command above it, through `$SHELL -lc` so the
 *    profile's PATH is there, and the click on that button is the only
 *    confirmation there is. Nothing needing elevation is ever offered; Rust
 *    refuses one too.
 * 4. The run's last event triggers a **recheck** — see `useAgentInstall`,
 *    which owns that so closing this dialog cannot skip it.
 *
 * The output stream is listened to in that hook rather than routed through
 * `useBackendEvents` into a store, which is this app's rule for background
 * jobs. An install is not one: it is a foreground thing the student is
 * watching, it has no meaning once Settings is left, and a package manager's
 * output is tens of kilobytes a store would then keep for the session. This
 * dialog is its surface, which is also why nothing here is a toast — this app
 * has none.
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
    // Follow the tail: an installer's last line is the one worth reading.
    const el = log.current;
    if (el) el.scrollTop = el.scrollHeight;
  }, [run?.lines, run?.result]);

  const copy = async (route: InstallRoute) => {
    // The tick is the whole answer: no toasts here, and a Copy that does
    // nothing visible is indistinguishable from one that failed.
    if (await copyText(route.command)) {
      setCopied(route.manager);
      window.setTimeout(() => setCopied((c) => (c === route.manager ? null : c)), 1200);
    }
  };

  // With something to run, only the routes that can be run are listed — a row
  // offering a command this machine has no tool for would be the dead control
  // this app does not ship. With nothing runnable, every command is listed and
  // copying one is the whole offer.
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

        {/* Before a run: every command that can be offered, verbatim. */}
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

        {/* During and after a run: the command, then its output as it lands. */}
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
                <CodeText>{run.lines.join("\n")}</CodeText>
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
