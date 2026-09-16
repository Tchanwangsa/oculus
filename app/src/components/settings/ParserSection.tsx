import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { cn } from "@/lib/utils";
import { tokenish } from "@/lib/parseState";
import { useParseStore } from "@/stores/parseStore";
import { Section, StatRow } from "@/pages/settings/section";

/** Mirrors `EngineOption` in `app/src-tauri/src/parse/commands.rs`. */
interface EngineOption {
  id: string;
  label: string;
  detail: string;
  available: boolean;
  /** Present only when `available` is false, and then always. */
  unavailable_reason: string | null;
}

/** Mirrors `ParseSettings` in `app/src-tauri/src/parse/commands.rs`. */
interface ParseSettings {
  engine: string;
  /** The endpoint in force — the override when there is one, else the default. */
  base_url: string;
  /** What the endpoint field offers when nothing is overridden. */
  default_base_url: string;
  overridden: boolean;
  /** The version *this app* writes, for the handshake below. */
  parser_version: number;
  credentials_ready: boolean;
  engines: EngineOption[];
}

/** Mirrors `LocalProbe` in `app/src-tauri/src/parse/commands.rs`. */
interface LocalProbe {
  state: "reachable" | "unreachable" | "version_mismatch";
  base_url: string;
  backend: string | null;
  parser_version: number | null;
  /** Always present when `state` is not `reachable`. */
  detail: string | null;
}

/**
 * Which MinerU reads the library's PDFs.
 *
 * The engine list, the labels and the reason an engine is unavailable all come
 * from Rust, so the page cannot offer something the backend would refuse, or
 * explain a refusal in different words.
 *
 * Unlike the embedding engine, switching this one costs nothing: markdown from
 * either backend is markdown, nothing already parsed is re-parsed and no index
 * is thrown away. So the control is a plain `onValueChange` with no
 * confirmation in front of it — a dialog raised over a change that destroys
 * nothing is how people learn to click through the one that matters.
 */
export function ParserSection() {
  const [settings, setSettings] = useState<ParseSettings | null>(null);
  const [switching, setSwitching] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // `null` is "not answered yet", and it is a state the UI must keep separate
  // from `false`. These two used to load in one `Promise.all`, so a failure in
  // *either* left this at its initial `false` — and the page then told someone
  // with a perfectly good token, in red, that they had none and nothing could
  // be parsed. A check that did not happen is not a negative answer.
  const [hasToken, setHasToken] = useState<boolean | null>(null);
  const [tokenCheckError, setTokenCheckError] = useState<string | null>(null);
  const [token, setToken] = useState("");
  const [tokenNote, setTokenNote] = useState<{ kind: "error" | "warn"; text: string } | null>(null);
  const [checkingToken, setCheckingToken] = useState(false);

  // The endpoint as typed, which is not the endpoint in force: an empty field
  // means "no override", and `default_base_url` is what it then stands for.
  const [urlDraft, setUrlDraft] = useState("");
  const [urlNote, setUrlNote] = useState<string | null>(null);
  const [savingUrl, setSavingUrl] = useState(false);

  const [probe, setProbe] = useState<LocalProbe | null>(null);
  const [probing, setProbing] = useState(false);
  const [probeError, setProbeError] = useState<string | null>(null);

  // The only live signal about the token now. The sidecar used to latch a
  // rejection in its own process and report it in the health this page polled;
  // the in-process client keeps no such state (see the note in
  // `app/src-tauri/src/mineru.rs`), so what is left is the app-wide latch the
  // parse events raise. That makes the "Expired" state session-scoped rather
  // than sticky — which is the honest scope, because the very next parse reads
  // the keychain afresh and a stale flag would outlive the problem.
  const latch = useParseStore((state) => state.latch);
  const clearLatch = useParseStore((state) => state.clearLatch);

  // Rust owns which endpoint is in force, so the field is re-seeded from every
  // answer it gives rather than kept as a second opinion.
  const applySettings = useCallback((next: ParseSettings) => {
    setSettings(next);
    setUrlDraft(next.overridden ? next.base_url : "");
  }, []);

  // Two independent questions, loaded independently. Neither can answer for
  // the other, and neither failing may be reported as the other's answer.
  useEffect(() => {
    let cancelled = false;

    invoke<ParseSettings>("parse_settings")
      .then((next) => {
        if (cancelled) return;
        applySettings(next);
        setError(null);
      })
      .catch((cause) => {
        console.error("parse settings failed", cause);
        if (cancelled) return;
        setError("Could not read the parser settings.");
      });

    // Asked whichever engine is selected: the answer decides what the cloud
    // row says the moment someone switches back to it, and a token check is
    // cheap next to a flash of "Checking…" over a row that was already known.
    invoke<boolean>("mineru_has_api_key")
      .then((present) => {
        if (cancelled) return;
        setHasToken(present);
        setTokenCheckError(null);
      })
      .catch((cause) => {
        console.error("MinerU token check failed", cause);
        if (cancelled) return;
        // Shown, not swallowed. The console is not somewhere a student looks,
        // and this is the whole reason the page was lying.
        setHasToken(null);
        setTokenCheckError(String(cause));
      });

    return () => {
      cancelled = true;
    };
  }, [applySettings]);

  // `url` probes an endpoint that has not been saved yet; without it Rust
  // probes the one in force.
  const runProbe = useCallback(async (url?: string) => {
    setProbing(true);
    setProbeError(null);
    try {
      const candidate = url?.trim();
      const next = await invoke<LocalProbe>(
        "parse_probe_local",
        candidate ? { url: candidate } : {},
      );
      setProbe(next);
    } catch (cause) {
      console.error("MinerU server probe failed", cause);
      setProbe(null);
      setProbeError(String(cause));
    } finally {
      setProbing(false);
    }
  }, []);

  // On mount when local is already selected, and again on every switch to it.
  // Switching away drops the verdict rather than leaving a stale one to be
  // read as the cloud's.
  useEffect(() => {
    if (settings?.engine !== "local") {
      setProbe(null);
      setProbeError(null);
      return;
    }
    void runProbe();
  }, [settings?.engine, runProbe]);

  const selected = settings?.engines.find((engine) => engine.id === settings.engine) ?? null;
  const unavailable = settings?.engines.filter((engine) => !engine.available) ?? [];

  const choose = async (engine: string) => {
    if (!settings || engine === settings.engine) return;
    setSwitching(true);
    setError(null);
    try {
      applySettings(await invoke<ParseSettings>("parse_set_engine", { engine }));
    } catch (cause) {
      console.error("parse engine change failed", cause);
      setError(String(cause));
    } finally {
      setSwitching(false);
    }
  };

  // An empty field clears the override, which is why this is never guarded on
  // a non-empty value the way the token is.
  const saveUrl = async () => {
    setUrlNote(null);
    setSavingUrl(true);
    try {
      applySettings(await invoke<ParseSettings>("parse_set_engine_url", { url: urlDraft.trim() }));
      await runProbe();
    } catch (cause) {
      console.error("parse endpoint change failed", cause);
      setUrlNote(String(cause));
    } finally {
      setSavingUrl(false);
    }
  };

  // Rust checks the token against MinerU before it reaches the keychain, so a
  // typo or an expired token is reported here rather than at the next parse.
  const saveToken = async () => {
    if (!token.trim()) return;
    setCheckingToken(true);
    setTokenNote(null);
    try {
      const verdict = await invoke<string>("mineru_set_api_key", { key: token.trim() });
      setToken("");
      setHasToken(true);
      // A token MinerU just accepted is the thing the latch was waiting on, so
      // lift it here rather than leaving the library "on hold" until some file
      // happens to parse. The sweep picks the outstanding files back up.
      clearLatch();
      setTokenNote(
        verdict === "unverified"
          ? { kind: "warn", text: "Saved, but MinerU was unreachable — it has not been checked." }
          : null,
      );
    } catch (cause) {
      setTokenNote({ kind: "error", text: String(cause) });
    } finally {
      setCheckingToken(false);
    }
  };

  const deleteToken = async () => {
    setTokenNote(null);
    try {
      await invoke("mineru_delete_api_key");
      setHasToken(false);
      setToken("");
    } catch (cause) {
      console.error("MinerU token removal failed", cause);
      setTokenNote({ kind: "error", text: String(cause) });
    }
  };

  const tokenExpired = hasToken === true && Boolean(latch && tokenish(latch.kind, latch.message));
  const isLocal = settings?.engine === "local";
  const isCloud = settings?.engine === "cloud";

  return (
    <Section
      title="PDF processing"
      description="Which MinerU reads your PDFs — its cloud service, or a server running on this Mac."
    >
      <div className="space-y-1">
        <div className="flex items-center justify-between gap-4 py-2">
          <div>
            <p className="text-xs text-foreground">Parser</p>
            <p className="text-[11px] text-muted-foreground">
              {selected?.detail ?? "Where PDFs are turned into per-page markdown."}
            </p>
          </div>
          <Select
            value={settings?.engine ?? ""}
            disabled={!settings || switching}
            onValueChange={(engine) => void choose(engine)}
          >
            <SelectTrigger aria-label="Parser" size="sm" className="h-7 w-48 text-xs">
              <SelectValue placeholder="—" />
            </SelectTrigger>
            <SelectContent>
              {(settings?.engines ?? []).map((engine) => (
                <SelectItem
                  key={engine.id}
                  value={engine.id}
                  disabled={!engine.available}
                  className="text-xs"
                >
                  <span>{engine.label}</span>
                  {engine.available ? null : (
                    <span className="text-[11px] text-muted-foreground">Unavailable</span>
                  )}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>

        {/* A disabled option with no reason beside it reads as a bug. Rust
            decides both the list and the refusal, so the sentence is its. */}
        {unavailable.map((engine) => (
          <p key={engine.id} className="text-[11px] leading-relaxed text-muted-foreground">
            {engine.label}: {engine.unavailable_reason}
          </p>
        ))}

        {/* The token belongs to the cloud engine and only to it. A local server
            needs no credential, and a key field standing under a backend that
            cannot use it is the kind of thing people paste secrets into. */}
        {isCloud ? (
          <div className="py-2">
            <div className="flex items-center justify-between gap-4">
              <div>
                <p className="text-xs text-foreground">MinerU API token</p>
                <p className="text-[11px] text-muted-foreground">
                  Stored in your Mac keychain, never in the library database.
                </p>
              </div>
              {hasToken === true && !tokenExpired ? (
                <div className="flex items-center gap-2">
                  <span className="text-xs text-success">Connected</span>
                  <Button variant="outline" size="xs" onClick={() => void deleteToken()}>
                    Remove
                  </Button>
                </div>
              ) : (
                <div className="flex items-center gap-2">
                  {tokenExpired ? <span className="text-xs text-warning">Expired</span> : null}
                  <Input
                    aria-label="MinerU API token"
                    type="password"
                    autoComplete="off"
                    value={token}
                    onChange={(event) => setToken(event.target.value)}
                    onKeyDown={(event) => {
                      if (event.key === "Enter") void saveToken();
                    }}
                    placeholder={tokenExpired ? "Paste new token" : "Paste token"}
                    className="h-7 w-44 text-xs"
                  />
                  <Button
                    size="xs"
                    disabled={!token.trim() || checkingToken}
                    onClick={() => void saveToken()}
                  >
                    {checkingToken ? "Checking…" : "Save"}
                  </Button>
                </div>
              )}
            </div>
            {tokenNote ? (
              <p
                className={cn(
                  "mt-2 text-[11px] leading-relaxed",
                  tokenNote.kind === "error" ? "text-destructive" : "text-warning",
                )}
              >
                {tokenNote.text}
              </p>
            ) : null}
            {tokenExpired ? (
              <p className="mt-2 text-[11px] leading-relaxed text-warning">
                MinerU refused this token during a parse. Nothing is being parsed until you paste a
                new one — a parse never falls back to another engine on its own.
              </p>
            ) : null}
            {hasToken === false && !tokenExpired ? (
              <p className="mt-2 text-[11px] text-warning">
                Without a token the cloud engine cannot parse anything, so no PDF is searchable
                or can be mentioned in chat.
              </p>
            ) : null}
            {hasToken === null ? (
              <p className="mt-2 text-[11px] leading-relaxed text-muted-foreground">
                {tokenCheckError
                  ? `Could not check whether a token is saved: ${tokenCheckError}`
                  : "Checking for a saved token…"}
              </p>
            ) : null}
          </div>
        ) : null}

        {isLocal ? (
          <div className="py-2">
            <div className="flex items-center justify-between gap-4">
              <div>
                <p className="text-xs text-foreground">Server address</p>
                <p className="text-[11px] text-muted-foreground">
                  Empty uses {settings?.default_base_url ?? "the default"}.
                </p>
              </div>
              <div className="flex items-center gap-2">
                <Input
                  aria-label="MinerU server address"
                  type="url"
                  autoComplete="off"
                  spellCheck={false}
                  value={urlDraft}
                  onChange={(event) => setUrlDraft(event.target.value)}
                  onKeyDown={(event) => {
                    if (event.key === "Enter") void saveUrl();
                  }}
                  placeholder={settings?.default_base_url ?? ""}
                  className="h-7 w-56 text-xs"
                />
                <Button size="xs" disabled={savingUrl} onClick={() => void saveUrl()}>
                  {savingUrl ? "Saving…" : "Save"}
                </Button>
              </div>
            </div>
            {urlNote ? (
              <p className="mt-2 text-[11px] leading-relaxed text-destructive">{urlNote}</p>
            ) : null}
          </div>
        ) : null}

        {isLocal ? (
          <div className="py-2">
            <div className="flex items-center justify-between gap-4">
              <div className="min-w-0">
                <p className="text-xs text-foreground">Server status</p>
                <ProbeLine probe={probe} probing={probing} probeError={probeError} />
              </div>
              <Button
                variant="outline"
                size="xs"
                disabled={probing}
                onClick={() => void runProbe(urlDraft)}
              >
                {probing ? "Checking…" : "Check"}
              </Button>
            </div>
          </div>
        ) : null}

        {isLocal && probe?.state === "reachable" ? (
          <StatRow
            label="Server"
            value={
              probe.parser_version == null
                ? (probe.backend ?? "—")
                : `${probe.backend ?? "MinerU"} · parser ${probe.parser_version}`
            }
          />
        ) : null}

        {/* Where the PDFs actually go, which is the whole difference between
            the two engines and is never left to be inferred from the label. */}
        {isCloud ? (
          <p className="pt-1 text-[11px] leading-relaxed text-muted-foreground">
            Lecture PDFs are uploaded to MinerU and its PRC-hosted OSS storage. Results may be
            cached by MinerU (its documented default cache tolerance is 15 minutes, not a
            deletion guarantee).
          </p>
        ) : null}
        {isLocal ? (
          <p className="pt-1 text-[11px] leading-relaxed text-muted-foreground">
            Lecture PDFs are read by the MinerU server on this Mac. Nothing is uploaded to
            MinerU’s cloud service or its PRC-hosted OSS storage, and nothing leaves this
            machine.
          </p>
        ) : null}

        {error ? (
          <p className="pt-1 text-[11px] leading-relaxed text-destructive">{error}</p>
        ) : null}
      </div>
    </Section>
  );
}

/**
 * Whether the local server is answering, in one line.
 *
 * Four states, not two. "Reachable" and "unreachable" are the obvious pair;
 * a version mismatch is a *third* problem with a different fix — the server
 * answered, it just speaks an API this build cannot use — and flattening it
 * into "not ok" sends someone to check a port that is fine. The fourth is a
 * probe that has not answered yet, which must never be drawn as a failure: no
 * answer is not a negative answer.
 *
 * **The sentence itself is Rust's, not this file's.** `detail` already says
 * what is wrong and what to do, and it distinguishes cases this side cannot
 * see — a server that answered but is still loading its models reads as
 * `unreachable` here, and a second sentence written from the state alone
 * would tell someone nothing answered while Rust told them something did.
 */
function ProbeLine({
  probe,
  probing,
  probeError,
}: {
  probe: LocalProbe | null;
  probing: boolean;
  probeError: string | null;
}) {
  if (probing) {
    return <p className="text-[11px] text-muted-foreground">Checking…</p>;
  }
  if (probeError) {
    return (
      <p className="text-[11px] leading-relaxed text-destructive">
        Could not check the server: {probeError}
      </p>
    );
  }
  if (!probe) {
    return <p className="text-[11px] text-muted-foreground">Not checked yet.</p>;
  }
  if (probe.state === "reachable") {
    return (
      <p className="text-[11px] leading-relaxed text-success">
        Answering at {probe.base_url}.
      </p>
    );
  }
  return (
    <p className="text-[11px] leading-relaxed text-warning">
      {probe.detail ?? `Nothing answered at ${probe.base_url}.`}
    </p>
  );
}
