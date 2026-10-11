import { Section, StatRow } from "@/components/settings/shared/section";
import { CredentialField } from "../shared/CredentialField";
import { EngineSelect } from "../shared/EngineSelect";
import { ReindexConfirmDialog } from "./ReindexConfirmDialog";
import { IndexRunRow } from "./embedding/IndexRunRow";
import { RunEstimate } from "./embedding/RunEstimate";
import { useEmbedActions } from "./embedding/useEmbedActions";
import { useEmbedSettings } from "./embedding/useEmbedSettings";
import { AllowanceMeter, PlanRow } from "./embedding/UsageMeter";

/**
 * The embedding backend, the account behind it, and the index it owns.
 *
 * Search runs against one model only, so switching engines discards every
 * stored vector: it is confirmed first (`ReindexConfirmDialog`), then Rust
 * clears the index and writes the setting in one call. The engine list and
 * unavailable reasons come from Rust so the page cannot offer what it refuses.
 */
export function EmbeddingSection() {
  const {
    settings,
    setSettings,
    error,
    setError,
    outstanding,
    estimate,
    estimating,
    loadEstimate,
    runRunning,
  } = useEmbedSettings();
  const {
    prompt,
    setPrompt,
    switching,
    key,
    setKey,
    keyNote,
    checkingKey,
    apply,
    choose,
    setBudget,
    saveKey,
    deleteKey,
  } = useEmbedActions({ settings, setSettings, setError, loadEstimate });

  const selected = settings?.engines.find((engine) => engine.id === settings.engine) ?? null;

  const usage = settings?.usage ?? null;

  return (
    <Section
      title="Search index"
      description="Search runs against the embedding model chosen here, and only that one."
    >
      <div className="space-y-1">
        <div className="flex items-center justify-between gap-4 py-2">
          <div>
            <p className="text-xs text-foreground">Embedding model</p>
            <p className="text-[11px] text-muted-foreground">
              {selected?.detail ?? "Where page images are turned into vectors."}
            </p>
          </div>
          <EngineSelect
            label="Embedding model"
            value={settings?.engine ?? ""}
            disabled={!settings || switching}
            engines={settings?.engines ?? []}
            onChange={choose}
          />
        </div>

        {settings?.credentials_error ? (
          <p className="text-[11px] leading-relaxed text-destructive" data-selectable>
            {settings.credentials_error}
          </p>
        ) : settings && !settings.credentials_ready && settings.engine === "cloud" ? (
          <p className="text-[11px] leading-relaxed text-warning">
            Nothing can be indexed or searched until a Voyage API key is saved below.
          </p>
        ) : null}

        <CredentialField
          label="Voyage API key"
          value={key}
          connected={settings?.credentials_ready ?? false}
          busy={checkingKey}
          placeholder="Paste key"
          onChange={setKey}
          onSave={() => void saveKey()}
          onRemove={() => void deleteKey()}
          note={keyNote}
        />

        <StatRow
          label="Pages indexed"
          value={settings ? settings.index.pages_embedded.toLocaleString() : "—"}
        />
        <StatRow
          label="Files indexed"
          value={settings ? settings.index.files_embedded.toLocaleString() : "—"}
        />
        {/* `index.model` is the space this build writes, not necessarily the
            space the stored vectors are in. */}
        <StatRow
          label="Search space"
          value={
            settings?.index.model && settings.index.dim
              ? `${settings.index.model} · ${settings.index.dim}d`
              : "—"
          }
        />
        {/* Files, not pages: a file is what the run walks. Vectors from another
            model count as not indexed. */}
        <StatRow
          label="Not indexed"
          value={
            outstanding == null
              ? "—"
              : outstanding === 0
                ? "None"
                : `${outstanding.toLocaleString()} file${outstanding === 1 ? "" : "s"}`
          }
        />

        {usage ? <PlanRow usage={usage} /> : null}
        {usage ? (
          <AllowanceMeter
            usage={usage}
            runPixels={runRunning || !estimate?.files ? 0 : estimate.pixels}
            onChange={(percent) => void setBudget(percent)}
          />
        ) : null}

        <IndexRunRow
          outstanding={outstanding}
          ready={Boolean(settings?.credentials_ready)}
        />

        {!runRunning && outstanding !== 0 ? (
          <RunEstimate estimate={estimate} estimating={estimating} usage={usage} />
        ) : null}

        {error ? (
          <p className="pt-1 text-[11px] leading-relaxed text-destructive">{error}</p>
        ) : null}
      </div>

      <ReindexConfirmDialog
        prompt={prompt}
        busy={switching}
        onConfirm={() => prompt && void apply(prompt.engine)}
        onCancel={() => setPrompt(null)}
      />
    </Section>
  );
}
