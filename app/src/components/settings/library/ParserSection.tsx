import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Switch } from "@/components/ui/switch";
import { CredentialField } from "../shared/CredentialField";
import { EngineSelect } from "../shared/EngineSelect";
import { tokenish } from "@/lib/pipeline/parseState";
import { expiryDate } from "@/lib/pipeline/resultCert";
import { useParseStore } from "@/stores/sync/parseStore";
import { Section, StatRow } from "@/components/settings/shared/section";
import { ProbeLine } from "./parser/ProbeLine";
import { useLocalProbe } from "./parser/useLocalProbe";
import { useParseActions } from "./parser/useParseActions";
import { useParseSettings } from "./parser/useParseSettings";

/**
 * Which MinerU reads the library's PDFs. Engine list and unavailable reasons
 * come from Rust. Switching destroys nothing (nothing is re-parsed), so unlike
 * the embedding engine there is no confirmation. No fallback between engines —
 * see docs/parsing.md: nothing catches a failed parse.
 */
export function ParserSection() {
  const {
    settings,
    setSettings,
    applySettings,
    error,
    setError,
    hasToken,
    setHasToken,
    tokenCheckError,
    urlDraft,
    setUrlDraft,
    cert,
  } = useParseSettings();
  const { probe, probing, probeError, runProbe } = useLocalProbe(settings?.engine);
  const {
    switching,
    token,
    setToken,
    tokenNote,
    checkingToken,
    urlNote,
    savingUrl,
    savingCert,
    certError,
    choose,
    saveUrl,
    saveToken,
    setAcceptExpired,
    deleteToken,
  } = useParseActions({ settings, setSettings, applySettings, setError, setHasToken, urlDraft, runProbe });

  // The only live signal about the token: the app-wide latch parse events
  // raise. "Expired" is session-scoped because each parse re-reads the keychain.
  const latch = useParseStore((state) => state.latch);

  const selected = settings?.engines.find((engine) => engine.id === settings.engine) ?? null;
  const unavailable = settings?.engines.filter((engine) => !engine.available) ?? [];

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
