import { useEffect, useMemo, useRef, useState } from "react";
import { ArrowSquareOut, CaretRight, CircleNotch } from "@phosphor-icons/react";
import { openUrl } from "@tauri-apps/plugin-opener";

import {
  formComplete,
  initialAnswers,
  opencodeOauthFinish,
  opencodeOauthStart,
  opencodeProviders,
  opencodeSetKey,
  visiblePrompts,
  type Answers,
  type AuthMethod,
  type Authorization,
  type OpencodeProvider,
  type OpencodeProviderList,
} from "@/lib/opencodeAuth";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";

/**
 * One dialog for every way into every opencode provider.
 *
 * It draws nothing it decided for itself. A method arrives from opencode as a
 * **form spec** — a kind, a label, and prompts that are text fields or selects,
 * some of them conditional — so this renders the spec and hands the answers
 * back by key. That is why `openai`'s three ways in, `github-copilot`'s
 * enterprise branch and the two hundred providers that simply take a key all
 * come out of the same component, and why a provider opencode adds next month
 * needs no change here.
 *
 * Three steps, and a provider with one obvious way in skips the first:
 *
 * 1. **Which way in**, when the provider declares more than one.
 * 2. **The form** — the method's prompts, plus the key field an `api` method
 *    always needs on top of them.
 * 3. **The browser**, for an `oauth` method. `auto` means opencode finishes the
 *    flow itself — the redirect lands on a loopback listener inside the app's
 *    own server, or a device code it polls for — so the app opens the URL and
 *    then watches for the credential to appear. `code` means the student pastes
 *    something back, and the field for it is the same generic row as any other.
 *
 * The link opens in the **system browser**: a student is far more likely to be
 * signed in to GitHub or OpenAI there than in this app's in-app one.
 *
 * Progress and failure stay in here. The app has no toasts, and a sign-in that
 * reported itself somewhere else would be a sign-in you had to go looking for.
 */
export function OpencodeConnectDialog({
  provider,
  onClose,
  onDone,
}: {
  provider: OpencodeProvider;
  onClose: () => void;
  /** The list as it stands after a successful connect. */
  onDone: (list: OpencodeProviderList) => void;
}) {
  const id = provider.id;
  const [chosen, setChosen] = useState<AuthMethod | null>(
    provider.methods.length === 1 ? provider.methods[0] : null,
  );
  const [answers, setAnswers] = useState<Answers>(() =>
    provider.methods.length === 1 ? initialAnswers(provider.methods[0]) : {},
  );
  const [key, setKey] = useState("");
  const [auth, setAuth] = useState<Authorization | null>(null);
  const [code, setCode] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  /** Set once the `auto` watch gives up, so the dialog stops claiming to be
   *  working while nothing is happening. */
  const [gaveUp, setGaveUp] = useState(false);

  const pick = (method: AuthMethod) => {
    setChosen(method);
    setAnswers(initialAnswers(method));
    setError(null);
  };

  const prompts = useMemo(
    () => (chosen ? visiblePrompts(chosen, answers) : []),
    [chosen, answers],
  );

  const submit = async () => {
    if (!chosen) return;
    setBusy(true);
    setError(null);
    try {
      if (chosen.kind === "api") {
        onDone(await opencodeSetKey(provider.id, chosen.index, key, answers));
        return;
      }
      const started = await opencodeOauthStart(provider.id, chosen.index, answers);
      setAuth(started);
      if (started.url) await openUrl(started.url);
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  const finishWithCode = async () => {
    if (!chosen) return;
    setBusy(true);
    setError(null);
    try {
      onDone(await opencodeOauthFinish(provider.id, chosen.index, code.trim() || null));
    } catch (e) {
      setError(String(e));
      setBusy(false);
    }
  };

  /**
   * The `auto` watch.
   *
   * opencode writes the credential itself — the redirect lands on its loopback
   * listener, or it polls a device code — and it announces nothing: there is no
   * auth event on the SSE stream the bridge is already connected to (checked
   * against the whole event union, and watched live through one), and
   * `connected` on a running instance does not change until the instance
   * re-reads its store. So the only honest signal is a refreshed read on a
   * timer, and it stops itself rather than polling behind a dialog nobody is
   * watching any more.
   *
   * The refresh behind each read disposes the instance, which was measured not
   * to disturb a flow in progress: the loopback listener opened before a
   * dispose is still bound after it, the event stream keeps heart-beating, and
   * sessions stay readable. It is a slow poll for that reason and not only for
   * the cost.
   */
  const watching = auth?.method === "auto" && !gaveUp;
  // The callback is read through a ref so the timer is not restarted every
  // time the section above re-renders and hands down a new closure.
  const latestDone = useRef(onDone);
  latestDone.current = onDone;

  useEffect(() => {
    if (!watching) return;
    let live = true;
    let timer = 0;
    const until = Date.now() + 3 * 60_000;
    const tick = async () => {
      try {
        const list = await opencodeProviders(true);
        if (!live) return;
        if (list.providers.find((p) => p.id === id)?.connected) {
          latestDone.current(list);
          return;
        }
      } catch {
        // A read that failed mid-flow is not evidence either way; the next
        // tick asks again and the deadline is what ends it.
      }
      if (!live) return;
      if (Date.now() < until) timer = window.setTimeout(() => void tick(), 3_000);
      else setGaveUp(true);
    };
    timer = window.setTimeout(() => void tick(), 3_000);
    return () => {
      live = false;
      window.clearTimeout(timer);
    };
  }, [watching, id]);

  const title = auth ? `Sign in to ${provider.name}` : `Connect ${provider.name}`;

  return (
    <Dialog open onOpenChange={(open) => !open && onClose()}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>{title}</DialogTitle>
          <DialogDescription>
            {auth
              ? auth.instructions ||
                "Finish signing in in your browser, then come back to this window."
              : chosen
                ? chosen.kind === "api"
                  ? "The key goes straight into opencode's credential store on this machine. Oculus does not keep a copy."
                  : "Signing in opens your browser. The credential is written by opencode, not by Oculus."
                : `${provider.name} offers more than one way in.`}
          </DialogDescription>
        </DialogHeader>

        {/* Step 1 — which way in. */}
        {!chosen && (
          <div className="flex flex-col gap-1">
            {provider.methods.map((m) => (
              <button
                key={m.index}
                type="button"
                onClick={() => pick(m)}
                className="flex cursor-pointer items-center justify-between gap-3 rounded-lg px-3 py-2 text-left text-[13px] text-foreground hover:bg-accent"
              >
                <span className="min-w-0 truncate">{m.label}</span>
                <CaretRight size={12} className="shrink-0 text-muted-foreground" />
              </button>
            ))}
          </div>
        )}

        {/* Step 2 — the method's own form. */}
        {chosen && !auth && (
          <div className="flex flex-col gap-3">
            {prompts.map((p) => (
              <div key={p.key} className="flex flex-col gap-1.5">
                <label className="text-xs text-muted-foreground" htmlFor={`oc-${p.key}`}>
                  {p.message}
                </label>
                {p.kind === "select" ? (
                  <Select
                    value={answers[p.key] ?? ""}
                    onValueChange={(v) => setAnswers((a) => ({ ...a, [p.key]: v }))}
                  >
                    <SelectTrigger id={`oc-${p.key}`} className="w-full">
                      <SelectValue placeholder="Choose one" />
                    </SelectTrigger>
                    <SelectContent>
                      {p.options.map((o) => (
                        <SelectItem key={o.value} value={o.value}>
                          {o.label}
                          {o.hint && <span className="ml-2 text-muted-foreground">{o.hint}</span>}
                        </SelectItem>
                      ))}
                    </SelectContent>
                  </Select>
                ) : (
                  <Input
                    id={`oc-${p.key}`}
                    value={answers[p.key] ?? ""}
                    placeholder={p.placeholder ?? undefined}
                    onChange={(e) => setAnswers((a) => ({ ...a, [p.key]: e.target.value }))}
                  />
                )}
              </div>
            ))}

            {/* An `api` method always needs a key on top of its prompts —
                `openai`'s "Manually enter API Key" declares none at all. */}
            {chosen.kind === "api" && (
              <div className="flex flex-col gap-1.5">
                <label className="text-xs text-muted-foreground" htmlFor="oc-key">
                  API key
                </label>
                <Input
                  id="oc-key"
                  type="password"
                  autoComplete="off"
                  spellCheck={false}
                  value={key}
                  placeholder="Paste the key from the provider"
                  onChange={(e) => setKey(e.target.value)}
                  onKeyDown={(e) => {
                    if (e.key === "Enter" && formComplete(chosen, answers, key)) void submit();
                  }}
                />
              </div>
            )}
          </div>
        )}

        {/* Step 3 — the browser flow. */}
        {chosen && auth && (
          <div className="flex flex-col gap-3">
            {auth.url && (
              <Button
                variant="outline"
                size="sm"
                className="self-start"
                onClick={() => void openUrl(auth.url)}
              >
                <ArrowSquareOut size={12} />
                Open the sign-in page again
              </Button>
            )}
            {auth.method === "auto" ? (
              <div className="flex items-center gap-2 text-xs text-muted-foreground">
                {gaveUp ? (
                  <span>
                    Still not signed in. Finish in the browser and reopen this section, or try
                    again.
                  </span>
                ) : (
                  <>
                    <CircleNotch size={12} className="animate-spin" />
                    <span>Waiting for {provider.name}…</span>
                  </>
                )}
              </div>
            ) : (
              <div className="flex flex-col gap-1.5">
                <label className="text-xs text-muted-foreground" htmlFor="oc-code">
                  Paste the code from the browser
                </label>
                <Input
                  id="oc-code"
                  value={code}
                  autoComplete="off"
                  spellCheck={false}
                  onChange={(e) => setCode(e.target.value)}
                  onKeyDown={(e) => {
                    if (e.key === "Enter" && code.trim()) void finishWithCode();
                  }}
                />
              </div>
            )}
          </div>
        )}

        {error && <p className="text-xs text-destructive">{error}</p>}

        <DialogFooter>
          <Button variant="outline" size="sm" onClick={onClose}>
            {auth?.method === "auto" && !gaveUp ? "Cancel" : "Close"}
          </Button>
          {chosen && !auth && (
            <Button
              size="sm"
              disabled={busy || !formComplete(chosen, answers, key)}
              onClick={() => void submit()}
            >
              {busy && <CircleNotch size={12} className="animate-spin" />}
              {chosen.kind === "api" ? "Save key" : "Open browser"}
            </Button>
          )}
          {chosen && auth?.method === "code" && (
            <Button size="sm" disabled={busy || !code.trim()} onClick={() => void finishWithCode()}>
              {busy && <CircleNotch size={12} className="animate-spin" />}
              Connect
            </Button>
          )}
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
