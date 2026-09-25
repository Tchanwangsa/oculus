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
 * One dialog for every way into every opencode provider. Each method arrives
 * from opencode as a form spec (text/select prompts, some conditional), which
 * this renders generically. Steps: pick a method (skipped when there is one),
 * fill its form (+ a key for `api`), then for `oauth` open the system browser —
 * `auto` means opencode finishes the flow itself and the app watches for the
 * credential; `code` means the user pastes a code back.
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
  /** Set once the `auto` watch gives up. */
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

  // The `auto` watch: opencode emits no auth event on SSE and `connected` only
  // changes after the instance re-reads its store, so poll a refreshed read
  // (which disposes the instance — safe mid-flow) until a deadline.
  const watching = auth?.method === "auto" && !gaveUp;
  // Via a ref so a parent re-render doesn't restart the timer.
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
        // Not evidence either way; the deadline ends it.
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

            {/* An `api` method always needs a key, even if it declares no prompt. */}
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
