import { useCallback, useEffect, useRef, useState } from "react";
import {
  CaretRight,
  CheckCircle,
  CircleNotch,
  Copy,
  Warning,
} from "@phosphor-icons/react";

import {
  harnessSignInCancel,
  harnessSignInCode,
  harnessSignInStart,
  providerLabel,
  signInFlow,
  SIGNIN_EVENT,
  type Provider,
  type SignInLine,
} from "@/lib/harness";
import { useTauriEvent } from "@/hooks/backend/useEvents";
import { signInAccount, useSignInStatus } from "@/hooks/agents/useSignInStatus";
import { navigateActive } from "@/lib/shell/tabRouters";
import { copyText, cn } from "@/lib/utils";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";

export interface SignInRun {
  provider: Provider;
  lines: string[];
  /** The authorize URL, once the CLI has printed one. */
  url: string | null;
  result: { ok: boolean; status: string } | null;
  error: string | null;
}

/**
 * The sign-in run, held by the dialog's opener (as `useAgentInstall` does), so
 * closing the dialog mid-flow neither strands the CLI nor skips the recheck.
 * `onFinished` (`useSignInStatus`'s `recheck`) runs however the flow ended.
 */
export function useSignIn(onFinished: () => void): {
  run: SignInRun | null;
  start: (provider: Provider) => void;
  submitCode: (code: string) => void;
  cancel: () => void;
  clear: () => void;
} {
  const [run, setRun] = useState<SignInRun | null>(null);

  /** Whose run is live. A ref, not read inside `setRun`: StrictMode runs
   *  updaters twice, which would post a single-use code twice. */
  const active = useRef<Provider | null>(null);

  // One listener for the host's life: `listen` attached per click resolves too
  // late and misses the first lines, where Claude prints its URL.
  useTauriEvent<SignInLine>(SIGNIN_EVENT, (e) => {
    const ev = e.payload;
    setRun((prev) => {
      if (!prev || prev.provider !== ev.provider) return prev;
      let next = prev;
      if (ev.url && !next.url) next = { ...next, url: ev.url };
      if (ev.line !== null) next = { ...next, lines: [...next.lines, ev.line] };
      if (ev.done) {
        next = { ...next, result: { ok: ev.ok ?? false, status: ev.status ?? "finished" } };
      }
      return next;
    });
    // Every mounted host hears every line; only the one that started it rechecks.
    if (ev.done && active.current === ev.provider) {
      active.current = null;
      onFinished();
    }
  });

  const start = useCallback((provider: Provider) => {
    active.current = provider;
    setRun({ provider, lines: [], url: null, result: null, error: null });
    harnessSignInStart(provider).catch((e) =>
      setRun((prev) => (prev && prev.provider === provider ? { ...prev, error: String(e) } : prev)),
    );
  }, []);

  const submitCode = useCallback((code: string) => {
    const provider = active.current;
    if (!provider) return;
    harnessSignInCode(provider, code).catch((e) =>
      setRun((prev) => (prev && prev.provider === provider ? { ...prev, error: String(e) } : prev)),
    );
  }, []);

  const cancel = useCallback(() => {
    const provider = active.current;
    active.current = null;
    if (provider) void harnessSignInCancel(provider).catch(() => {});
    setRun(null);
  }, []);

  const clear = useCallback(() => {
    active.current = null;
    setRun(null);
  }, []);

  return { run, start, submitCode, cancel, clear };
}

/**
 * Signs a CLI agent in via its own login subcommand, which writes the CLI's own
 * store — Oculus never sees the token. The flow is the provider's
 * (`ProviderInfo.signIn` in `app/src/lib/harness/index.ts`): `code` (Claude: paste the
 * callback code to stdin), `callback` (Codex: loopback server, nothing to type),
 * or null (opencode: per-provider credentials live in Settings → opencode).
 * The URL gets a Copy button in case Rust's browser `open` silently failed.
 */
export function SignInDialog({
  provider,
  run,
  onStart,
  onCode,
  onCancel,
  onClose,
}: {
  provider: Provider;
  run: SignInRun | null;
  onStart: () => void;
  onCode: (code: string) => void;
  onCancel: () => void;
  onClose: () => void;
}) {
  const label = providerLabel(provider);
  const flow = signInFlow(provider);
  const { statuses } = useSignInStatus();
  const account = signInAccount(statuses, provider);

  const [code, setCode] = useState("");
  const [copied, setCopied] = useState(false);
  const [showLog, setShowLog] = useState(false);

  // A different provider's dialog must not inherit the typed code.
  useEffect(() => setCode(""), [provider]);

  const live = !!run && !run.result;
  const url = run?.url ?? null;

  const copy = async () => {
    if (!url) return;
    if (await copyText(url)) {
      setCopied(true);
      window.setTimeout(() => setCopied(false), 1200);
    }
  };

  const submit = () => {
    const c = code.trim();
    if (!c) return;
    setCode("");
    onCode(c);
  };

  if (flow === null) {
    return (
      <Dialog open onOpenChange={(open) => !open && onClose()}>
        <DialogContent className="sm:max-w-md">
          <DialogHeader>
            <DialogTitle>Sign in to {label}</DialogTitle>
            <DialogDescription>
              opencode holds a credential per provider rather than one account of its own, so
              signing in means connecting Anthropic, OpenAI or whichever provider you run it
              on — which is a list, and it lives in Settings.
            </DialogDescription>
          </DialogHeader>
          <DialogFooter>
            <Button variant="outline" size="sm" onClick={onClose}>
              Close
            </Button>
            <Button
              size="sm"
              onClick={() => {
                onClose();
                navigateActive("/settings/opencode");
              }}
            >
              Open Settings → opencode
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    );
  }

  return (
    <Dialog open onOpenChange={(open) => !open && onClose()}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>Sign in to {label}</DialogTitle>
          <DialogDescription>
            {label}&rsquo;s own sign-in page opens in your browser. Oculus never sees the
            credential — it is the CLI&rsquo;s own store that is written, the same one your
            terminal reads.
          </DialogDescription>
        </DialogHeader>

        {live && (
          <div className="flex flex-col gap-3">
            {url && (
              <div className="flex items-center gap-2 rounded-lg border border-border-subtle bg-surface px-2.5 py-2">
                <span className="min-w-0 flex-1 truncate text-xs text-muted-foreground" title={url}>
                  {url}
                </span>
                <Button variant="ghost" size="xs" className="shrink-0" onClick={() => void copy()}>
                  {copied ? <CheckCircle size={12} /> : <Copy size={12} />}
                  {copied ? "Copied" : "Copy"}
                </Button>
              </div>
            )}

            {/* The CLI waits on stdin for the code the callback page shows. */}
            {flow === "code" && live && url && (
              <div className="flex items-center gap-2">
                <Input
                  value={code}
                  autoFocus
                  onChange={(e) => setCode(e.target.value)}
                  onKeyDown={(e) => {
                    if (e.key === "Enter") {
                      e.preventDefault();
                      submit();
                    }
                  }}
                  placeholder="Paste the code from the browser"
                  aria-label="Authorization code"
                />
                <Button size="sm" className="shrink-0" disabled={!code.trim()} onClick={submit}>
                  Submit
                </Button>
              </div>
            )}

            {flow === "callback" && live && (
              <div className="flex items-center gap-2 text-xs text-muted-foreground">
                <CircleNotch size={12} className="animate-spin" />
                <span>
                  {url
                    ? "Waiting for the browser to come back…"
                    : "Starting the sign-in and opening your browser…"}
                </span>
              </div>
            )}

            {flow === "code" && live && !url && (
              <div className="flex items-center gap-2 text-xs text-muted-foreground">
                <CircleNotch size={12} className="animate-spin" />
                <span>Starting the sign-in and opening your browser…</span>
              </div>
            )}
          </div>
        )}

        {run?.result && (
          <div
            className={cn(
              "flex items-start gap-2 text-xs",
              run.result.ok ? "text-success" : "text-destructive",
            )}
          >
            {run.result.ok ? (
              <CheckCircle size={13} className="mt-px shrink-0" />
            ) : (
              <Warning size={13} className="mt-px shrink-0" />
            )}
            <span>
              {run.result.ok
                ? account
                  ? `Signed in as ${account}.`
                  : `Signed in to ${label}.`
                : run.result.status}
            </span>
          </div>
        )}

        {run?.error && <p className="text-xs text-destructive">{run.error}</p>}

        {!!run?.lines.length && (
          <div>
            <button
              type="button"
              onClick={() => setShowLog((s) => !s)}
              className="flex cursor-pointer items-center gap-1 text-xs text-muted-foreground transition-colors hover:text-foreground"
            >
              <CaretRight
                size={11}
                className={cn("transition-transform", showLog && "rotate-90")}
              />
              {showLog ? "Hide output" : "Show output"}
            </button>
            {showLog && (
              <div className="mt-1.5 max-h-48 overflow-y-auto whitespace-pre-wrap break-words rounded-lg border border-border-subtle bg-surface p-2.5 text-[11.5px] leading-relaxed text-muted-foreground">
                {run.lines.join("\n")}
              </div>
            )}
          </div>
        )}

        <DialogFooter>
          {live ? (
            <>
              <Button variant="outline" size="sm" onClick={onClose}>
                Hide
              </Button>
              <Button variant="outline" size="sm" onClick={onCancel}>
                Cancel sign-in
              </Button>
            </>
          ) : (
            <>
              <Button variant="outline" size="sm" onClick={onClose}>
                Close
              </Button>
              {!run?.result && (
                <Button size="sm" onClick={onStart}>
                  {run ? "Try again" : "Sign in"}
                </Button>
              )}
            </>
          )}
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
