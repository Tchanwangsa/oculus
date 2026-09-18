import { useCallback, useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
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
import { signInAccount, useSignInStatus } from "@/hooks/useSignInStatus";
import { navigateActive } from "@/lib/tabRouters";
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
 * The run itself, held by whatever **opened** the dialog rather than by the
 * dialog — the shape `useAgentInstall` settled on next door, for the same
 * reason and one more.
 *
 * The same one: a student who closes the dialog mid-flow has not cancelled
 * anything. The browser is still open on the agent's sign-in page, the child
 * is still waiting, and if the stream lived in the dialog, closing it would
 * strand both — and, worse, skip the recheck that turns "signed out" back into
 * an account name. Held a level up, the dialog is a view of this and reopening
 * it resumes the same run.
 *
 * The one more: the dialog is opened from three places — a timeline row, the
 * composer's warning line, Settings → AI — and a run started from one of them
 * has to finish for all three. `onFinished` is `useSignInStatus`'s `recheck`,
 * and it runs however the flow ended, because a failure is a state worth
 * re-reading too.
 */
export function useSignIn(onFinished: () => void): {
  run: SignInRun | null;
  start: (provider: Provider) => void;
  submitCode: (code: string) => void;
  cancel: () => void;
  clear: () => void;
} {
  const [run, setRun] = useState<SignInRun | null>(null);

  const finished = useRef(onFinished);
  finished.current = onFinished;

  /** Whose run is live, for the two commands that act on it. A ref rather
   *  than a read inside `setRun`: a state updater has to stay pure, and under
   *  StrictMode's double invocation an invoke fired from one would go out
   *  twice — which for `harness_sign_in_code` means posting the same
   *  single-use code to the CLI a second time. */
  const active = useRef<Provider | null>(null);

  // One listener for the life of the host, not one per run: attached on the
  // click it would miss the first lines — and on Claude the URL is on one of
  // them — since `listen` resolves a tick after the child is already talking.
  useEffect(() => {
    const un = listen<SignInLine>(SIGNIN_EVENT, (e) => {
      const ev = e.payload;
      setRun((prev) => {
        if (!prev || prev.provider !== ev.provider) return prev;
        let next = prev;
        // The URL arrives once, on the first line that carries one, so it is
        // kept on the run rather than hunted back out of the log.
        if (ev.url && !next.url) next = { ...next, url: ev.url };
        if (ev.line !== null) next = { ...next, lines: [...next.lines, ev.line] };
        if (ev.done) {
          next = { ...next, result: { ok: ev.ok ?? false, status: ev.status ?? "finished" } };
        }
        return next;
      });
      // Only the host that started this run rechecks. All three mount their
      // own `useSignIn` and every listener hears every line, so an unguarded
      // call here would fire `recheck` once per host — two extra rounds of
      // CLI spawns for one sign-in.
      if (ev.done && active.current === ev.provider) {
        active.current = null;
        finished.current();
      }
    });
    return () => {
      void un.then((f) => f());
    };
  }, []);

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
 * Signing a CLI agent back in, from wherever the app noticed it was signed
 * out: the timeline row a failed turn left behind, the composer's line above
 * the box, or the agent's row in Settings → AI.
 *
 * What it does *not* do is hold a credential. Rust spawns the CLI's own login
 * subcommand and the browser flow writes to that CLI's own store — the same
 * store the student's terminal reads — so Oculus never sees the token. That is
 * the first sentence of the dialog, because it is the question a student has
 * when an app offers to sign them in to something.
 *
 * Three arms, and they are the providers' own flows rather than a shape
 * imposed here (`ProviderInfo.signIn` in `app/src/lib/harness.ts` carries
 * which is which):
 *
 * - **Claude** prints the authorize URL and then blocks reading a pasted
 *   authorization code off stdin, so there is a field and a Submit under the
 *   link. The student authorizes in the browser, copies what the callback page
 *   shows, and pastes it back.
 * - **Codex** starts a loopback server on :1455 and finishes by itself the
 *   moment the browser callback lands, so there is nothing to type and the
 *   dialog only says it is waiting.
 * - **opencode** is not signed in at all here, deliberately. Its credentials
 *   are per *provider*, not per CLI, and the whole surface for them — the
 *   catalogue, the form specs, the OAuth flows — already exists in
 *   Settings → AI (`OpencodeProvidersSection.tsx`). A second path to the same
 *   store would be a second answer to "am I signed in", so this arm is one
 *   sentence and a way there.
 *
 * The URL is shown with Copy even though Rust opens it in the system browser
 * itself: an `open` that silently failed would otherwise be a dead end with a
 * spinner on it. Body font, not monospace — a URL is not code.
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

  // A run that finished while the dialog was closed still has its lines; a
  // fresh open of a different provider's dialog must not inherit the field.
  useEffect(() => setCode(""), [provider]);

  const live = !!run && !run.result;
  const url = run?.url ?? null;

  const copy = async () => {
    if (!url) return;
    // The tick is the whole answer: no toasts here, and a Copy that does
    // nothing visible is indistinguishable from one that failed.
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
                navigateActive("/settings/ai");
              }}
            >
              Open Settings → AI
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

            {/* Claude's flow is only half done when the browser is open: the
                CLI is sitting on stdin waiting for the code the callback page
                shows. Offered once the URL is out, because before that there
                is nothing to have copied. */}
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

            {/* Codex finishes itself — the browser comes back to its loopback
                server on :1455 — so there is nothing to type here. */}
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

        {/* The CLI's own output. Collapsed by default, like the install
            dialog's log: it is what to read when the flow went wrong, and
            noise when it did not. */}
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
              {/* The start button shares the footer row rather than sitting
                  above it: before a run there is nothing else in the body, and
                  a lone button on its own line above Close read as two
                  separate decisions. */}
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
