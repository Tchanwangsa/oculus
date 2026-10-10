import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Switch } from "@/components/ui/switch";
import { CredentialField } from "./CredentialField";
import { EngineSelect, type EngineOption } from "./EngineSelect";
import { tokenish } from "@/lib/parseState";
import { expiryDate, resultCertState, type ResultCertState } from "@/lib/resultCert";
import { useParseStore } from "@/stores/parseStore";
import { Section, StatRow } from "@/pages/settings/section";

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
  /** The keychain or oculus-keyd refused to say whether a token is saved. */
  credentials_error: string | null;
  /** Download results through MinerU's expired CDN certificate. */
  accept_expired_result_cert: boolean;
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
 * Which MinerU reads the library's PDFs. Engine list and unavailable reasons
 * come from Rust. Switching destroys nothing (nothing is re-parsed), so unlike
 * the embedding engine there is no confirmation. No fallback between engines —
 * see docs/parsing.md: nothing catches a failed parse.
 */
export function ParserSection() {
  const [settings, setSettings] = useState<ParseSettings | null>(null);
  const [switching, setSwitching] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // `null` is "not answered yet" and must stay distinct from `false`: a check
  // that failed is not a missing token.
  const [hasToken, setHasToken] = useState<boolean | null>(null);
  const [tokenCheckError, setTokenCheckError] = useState<string | null>(null);
  const [token, setToken] = useState("");
  const [tokenNote, setTokenNote] = useState<{ kind: "error" | "warn"; text: string } | null>(null);
  const [checkingToken, setCheckingToken] = useState(false);

  // The endpoint as typed; empty means "no override" (`default_base_url`).
  const [urlDraft, setUrlDraft] = useState("");
  const [urlNote, setUrlNote] = useState<string | null>(null);
  const [savingUrl, setSavingUrl] = useState(false);

  const [cert, setCert] = useState<ResultCertState | null>(null);
  const [savingCert, setSavingCert] = useState(false);
  const [certError, setCertError] = useState<string | null>(null);

  const [probe, setProbe] = useState<LocalProbe | null>(null);
  const [probing, setProbing] = useState(false);
  const [probeError, setProbeError] = useState<string | null>(null);

  // The only live signal about the token: the app-wide latch parse events
  // raise. "Expired" is session-scoped because each parse re-reads the keychain.
  const latch = useParseStore((state) => state.latch);
  const clearLatch = useParseStore((state) => state.clearLatch);

  // Rust owns the endpoint in force, so re-seed the field from every answer.
  const applySettings = useCallback((next: ParseSettings) => {
    setSettings(next);
    setUrlDraft(next.overridden ? next.base_url : "");
  }, []);

  // Loaded independently so one failing is never read as the other's answer.
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

    // Asked whichever engine is selected, so switching to cloud shows no flash.
    invoke<boolean>("mineru_has_api_key")
      .then((present) => {
        if (cancelled) return;
        setHasToken(present);
        setTokenCheckError(null);
      })
      .catch((cause) => {
        console.error("MinerU token check failed", cause);
        if (cancelled) return;
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

  // Probe whenever local is selected; switching away drops the verdict.
  useEffect(() => {
    if (settings?.engine !== "local") {
      setProbe(null);
      setProbeError(null);
      return;
    }
    void runProbe();
  }, [settings?.engine, runProbe]);

  // A bare handshake with the result CDN, so the warning is current on open.
  useEffect(() => {
    if (settings?.engine !== "cloud") return;
    let cancelled = false;
    resultCertState(true)
      .then((next) => {
        if (!cancelled) setCert(next);
      })
      .catch((cause) => console.error("result certificate probe failed", cause));
    return () => {
      cancelled = true;
    };
  }, [settings?.engine, settings?.accept_expired_result_cert]);

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

  // Not guarded on non-empty: an empty field clears the override.
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

  // Rust checks the token against MinerU before storing it in the keychain.
  const saveToken = async () => {
    if (!token.trim()) return;
    setCheckingToken(true);
    setTokenNote(null);
    try {
      const verdict = await invoke<string>("mineru_set_api_key", { key: token.trim() });
      setToken("");
      setHasToken(true);
      setSettings((prev) => (prev ? { ...prev, credentials_ready: true, credentials_error: null } : prev));
      // An accepted token lifts the latch; the sweep resumes outstanding files.
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

  const setAcceptExpired = async (accept: boolean) => {
    setSavingCert(true);
    setCertError(null);
    try {
      applySettings(
        await invoke<ParseSettings>("parse_set_accept_expired_result_cert", { accept }),
      );
    } catch (cause) {
      console.error("result certificate setting failed", cause);
      setCertError(String(cause));
    } finally {
      setSavingCert(false);
    }
  };

  const deleteToken = async () => {
    setTokenNote(null);
    try {
      await invoke("mineru_delete_api_key");
      setHasToken(false);
      setToken("");
      setSettings((prev) => (prev ? { ...prev, credentials_ready: false, credentials_error: null } : prev));
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
          <EngineSelect
            label="Parser"
            value={settings?.engine ?? ""}
            disabled={!settings || switching}
            engines={settings?.engines ?? []}
            onChange={(engine) => void choose(engine)}
          />
        </div>

        {unavailable.map((engine) => (
          <p key={engine.id} className="text-[11px] leading-relaxed text-muted-foreground">
            {engine.label}: {engine.unavailable_reason}
          </p>
        ))}

        {isCloud && settings?.credentials_error ? (
          <p className="text-[11px] leading-relaxed text-destructive" data-selectable>
            {settings.credentials_error}
          </p>
        ) : null}

        {/* Cloud only: a local server needs no credential. */}
        {isCloud ? (
          <CredentialField
            label="MinerU API token"
            value={token}
            connected={hasToken === true && !tokenExpired}
            busy={checkingToken}
            placeholder={tokenExpired ? "Paste new token" : "Paste token"}
            onChange={setToken}
            onSave={() => void saveToken()}
            onRemove={() => void deleteToken()}
            status={tokenExpired ? <span className="text-xs text-warning">Expired</span> : null}
            note={tokenNote}
          >
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
            {/* The refusal itself is shown above, from the settings answer. */}
            {hasToken === null && !settings?.credentials_error ? (
              <p className="mt-2 text-[11px] leading-relaxed text-muted-foreground">
                {tokenCheckError
                  ? `Could not check whether a token is saved: ${tokenCheckError}`
                  : "Checking for a saved token…"}
              </p>
            ) : null}
          </CredentialField>
        ) : null}

        {isCloud ? (
          <div className="py-2">
            <div className="flex items-center justify-between gap-4">
              <label htmlFor="accept-expired-cert">
                <p className="text-xs text-foreground">Accept an expired download certificate</p>
                <p className="text-[11px] text-muted-foreground">
                  Only for MinerU's result server, and only while its certificate has expired.
                  Everything else about it is still checked.
                </p>
              </label>
              <Switch
                id="accept-expired-cert"
                checked={settings?.accept_expired_result_cert ?? true}
                disabled={!settings || savingCert}
                onCheckedChange={(checked) => void setAcceptExpired(checked)}
                className="shrink-0"
              />
            </div>
            {cert?.certificate === "expired" ? (
              <p className="mt-2 text-[11px] leading-relaxed text-warning">
                {cert.bypassing
                  ? `MinerU's download server certificate expired ${expiryDate(cert)}. Parse results are downloading anyway; this stops on its own once MinerU renews it.`
                  : `MinerU's download server certificate expired ${expiryDate(cert)}. Every parse will fail until MinerU renews it or this is turned on.`}
              </p>
            ) : null}
            {certError ? (
              <p className="mt-2 text-[11px] leading-relaxed text-destructive">{certError}</p>
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
 * Whether the local server is answering, in one line. The failure sentence is
 * Rust's `detail`, which distinguishes cases the state alone cannot (e.g. a
 * server still loading models reads as `unreachable`).
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
