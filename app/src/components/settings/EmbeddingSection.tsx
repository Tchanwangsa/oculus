import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";
import { ArrowSquareOut, Info } from "@phosphor-icons/react";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { CredentialField } from "./CredentialField";
import { EngineSelect, type EngineOption } from "./EngineSelect";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { cn } from "@/lib/utils";
import { embedReady, getUnembeddedPdfs } from "@/lib/retrieval";
import { Progress } from "@/components/ui/progress";
import { useIndexStore, type IndexProgress, type IndexState } from "@/stores/indexStore";
import { Section, StatRow } from "@/pages/settings/section";
import { ReindexConfirmDialog, type ReindexPrompt } from "./ReindexConfirmDialog";

/**
 * Mirrors `VoyageUsage` in `app/src-tauri/src/embed/commands.rs`. Oculus's own
 * tally (`voyage-usage.json`), not Voyage's — Voyage has no usage endpoint.
 */
interface VoyageUsage {
  /** `"free"`, `"paid"` or `"unknown"` — see `plan_source` before believing it. */
  plan: string;
  plan_source: string;
  rpm: number;
  tpm: number;
  learned_at: number;
  requests: number;
  tokens: number;
  pixels: number;
  free_pixels: number;
  free_pixels_left: number;
  usd_per_billion_pixels: number;
  /** The spend guard, as a percentage of the free grant. 0 is off. */
  stop_at_percent: number;
  quota_latched: boolean;
}

/** Mirrors `Bucket` in `app/src-tauri/src/embed/estimate.rs`. */
interface Bucket {
  label: string;
  files: number;
  pages: number;
}

/** Mirrors `EmbedEstimate` in `app/src-tauri/src/embed/estimate.rs`. */
interface EmbedEstimate {
  files: number;
  pages: number;
  unreadable: number;
  pixels: number;
  tokens: number;
  requests: number;
  kinds: Bucket[];
  billable_pixels: number;
  cost_usd: number;
  free_pixels_left: number;
  /** Where the spend guard would cut this run short, if it would. */
  stops_after_pages: number | null;
  seconds: number;
  seconds_tier1: number;
  tier_rpm: number;
  tier_tpm: number;
  tier_free: boolean;
  tier_source: string;
}

/** Mirrors `EmbedSettings` in `app/src-tauri/src/embed/commands.rs`. */
interface EmbedSettings {
  engine: string;
  base_url: string;
  model: string;
  dim: number;
  credentials_ready: boolean;
  engines: EngineOption[];
  index: {
    files_embedded: number;
    pages_embedded: number;
    pages_with_markdown: number;
    model: string | null;
    dim: number | null;
    files_stored: number;
    pages_stored: number;
    pages_stale: number;
    stale_models: string[];
  };
  /** `null` for a local engine: no allowance, no tier, nothing to guard. */
  usage: VoyageUsage | null;
}

/** 5,180,000,000 → "5.2B". */
function si(value: number): string {
  if (value >= 1e9) return `${(value / 1e9).toFixed(1)}B`;
  if (value >= 1e6) return `${(value / 1e6).toFixed(1)}M`;
  if (value >= 1e3) return `${Math.round(value / 1e3)}K`;
  return value.toLocaleString();
}

/** A rough duration for a sentence — the estimate has no minute precision. */
function roughly(seconds: number): string {
  if (seconds < 90) return "under a minute";
  const minutes = seconds / 60;
  if (minutes < 90) return `about ${Math.round(minutes)} minutes`;
  const hours = minutes / 60;
  if (hours < 36) return `about ${Math.round(hours)} hours`;
  return `about ${Math.round(hours / 24)} days`;
}

/**
 * The embedding backend, the account behind it, and the index it owns.
 *
 * Search runs against one model only, so switching engines discards every
 * stored vector: it is confirmed first (`ReindexConfirmDialog`), then Rust
 * clears the index and writes the setting in one call. The engine list and
 * unavailable reasons come from Rust so the page cannot offer what it refuses.
 */
export function EmbeddingSection() {
  const [settings, setSettings] = useState<EmbedSettings | null>(null);
  const [prompt, setPrompt] = useState<(ReindexPrompt & { engine: string }) | null>(null);
  const [switching, setSwitching] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const [key, setKey] = useState("");
  const [keyNote, setKeyNote] = useState<{ kind: "error" | "warn"; text: string } | null>(null);
  const [checkingKey, setCheckingKey] = useState(false);

  // Files a run would touch — the same query the run walks, since the index
  // counts above can't give it (stale pages, partly-embedded files).
  const [outstanding, setOutstanding] = useState<number | null>(null);

  // Separate from `settings` because it is slow (Rust opens every outstanding PDF).
  const [estimate, setEstimate] = useState<EmbedEstimate | null>(null);
  const [estimating, setEstimating] = useState(false);

  const run = useIndexStore();

  // One sweep at a time: pdfium is a single process-wide session (`raster.rs`),
  // so concurrent calls just queue. A request during a sweep is remembered and
  // re-run after, so a changed spend limit is never quoted with a stale cut-off.
  const sweeping = useRef(false);
  const resweep = useRef(false);
  const loadEstimate = useCallback(() => {
    if (sweeping.current) {
      resweep.current = true;
      return;
    }
    sweeping.current = true;
    setEstimating(true);
    invoke<EmbedEstimate>("embed_estimate")
      .then(setEstimate)
      .catch((cause) => {
        console.error("embed estimate failed", cause);
        setEstimate(null);
      })
      .finally(() => {
        sweeping.current = false;
        setEstimating(false);
        if (resweep.current) {
          resweep.current = false;
          loadEstimate();
        }
      });
  }, []);

  const reload = useCallback(() => {
    invoke<EmbedSettings>("embed_settings")
      .then(setSettings)
      .catch((cause) => {
        console.error("embed settings failed", cause);
        setError("Could not read the embedding settings.");
      });
    getUnembeddedPdfs()
      .then((files) => setOutstanding(files.length))
      .catch(() => setOutstanding(null));
    loadEstimate();
  }, [loadEstimate]);

  useEffect(() => {
    let cancelled = false;
    invoke<EmbedSettings>("embed_settings")
      .then((next) => {
        if (!cancelled) setSettings(next);
      })
      .catch((cause) => {
        console.error("embed settings failed", cause);
        if (!cancelled) setError("Could not read the embedding settings.");
      });
    getUnembeddedPdfs()
      .then((files) => {
        if (!cancelled) setOutstanding(files.length);
      })
      .catch(() => {
        if (!cancelled) setOutstanding(null);
      });
    loadEstimate();
    return () => {
      cancelled = true;
    };
  }, [loadEstimate]);

  // A finished run moves every number here — including the detected tier the
  // estimate's hours depend on — so re-read them.
  useEffect(() => {
    if (!run.running && (run.result || run.error)) reload();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [run.running]);

  const selected = settings?.engines.find((engine) => engine.id === settings.engine) ?? null;

  const apply = async (engine: string) => {
    setSwitching(true);
    setError(null);
    try {
      setSettings(await invoke<EmbedSettings>("embed_set_engine", { engine }));
      setPrompt(null);
      loadEstimate();
    } catch (cause) {
      console.error("embed engine change failed", cause);
      setError(String(cause));
    } finally {
      setSwitching(false);
    }
  };

  // An empty index has nothing to lose, so it skips the confirmation.
  const choose = (engine: string) => {
    if (!settings || engine === settings.engine) return;
    const { pages_embedded, files_embedded, model } = settings.index;
    if (pages_embedded === 0) {
      void apply(engine);
      return;
    }
    setPrompt({
      engine,
      to: settings.engines.find((option) => option.id === engine)?.label ?? engine,
      from: model,
      vectors: pages_embedded,
      files: files_embedded,
    });
  };

  // The guard moves where the run stops, so re-measure the estimate.
  const setBudget = async (percent: number) => {
    try {
      setSettings(await invoke<EmbedSettings>("embed_set_budget", { percent }));
      loadEstimate();
    } catch (cause) {
      console.error("embed budget change failed", cause);
      setError(String(cause));
    }
  };

  // Rust checks the key against Voyage before storing it in the keychain.
  const saveKey = async () => {
    if (!key.trim()) return;
    setCheckingKey(true);
    setKeyNote(null);
    try {
      const verdict = await invoke<string>("voyage_set_api_key", { key: key.trim() });
      setKey("");
      setSettings((prev) => (prev ? { ...prev, credentials_ready: true } : prev));
      // Re-asked rather than assumed: the engine also decides readiness.
      void embedReady().then((ready) => useIndexStore.getState().setReady(ready));
      setKeyNote(
        verdict === "unverified"
          ? { kind: "warn", text: "Saved, but Voyage was unreachable — it has not been checked." }
          : null,
      );
    } catch (cause) {
      setKeyNote({ kind: "error", text: String(cause) });
    } finally {
      setCheckingKey(false);
    }
  };

  const deleteKey = async () => {
    setKeyNote(null);
    try {
      await invoke("voyage_delete_api_key");
      setKey("");
      setSettings((prev) => (prev ? { ...prev, credentials_ready: false } : prev));
      useIndexStore.getState().setReady(false);
    } catch (cause) {
      console.error("Voyage key removal failed", cause);
      setKeyNote({ kind: "error", text: String(cause) });
    }
  };

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

        {settings && !settings.credentials_ready && settings.engine === "cloud" ? (
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
            runPixels={run.running || !estimate?.files ? 0 : estimate.pixels}
            onChange={(percent) => void setBudget(percent)}
          />
        ) : null}

        <IndexRunRow
          outstanding={outstanding}
          ready={Boolean(settings?.credentials_ready)}
          run={run}
        />

        {!run.running && outstanding !== 0 ? (
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

/** Where a payment method is added. Linked, not described. */
const VOYAGE_DASHBOARD = "https://dashboard.voyageai.com/";

/**
 * What plan this account is on. May be "unknown": limits are only detected
 * from 429 bodies during a run (`embed/voyage/ledger.rs`).
 */
function PlanRow({ usage }: { usage: VoyageUsage }) {
  const perMinute = `${si(usage.tpm)} tokens/min`;
  return (
    <StatRow
      label="Voyage plan"
      value={
        usage.plan === "free"
          ? `No payment method · ${perMinute}`
          : usage.plan === "paid"
            ? `Payment method on file · ${perMinute}`
            : "Not measured yet"
      }
      hint={
        usage.plan === "unknown"
          ? "Voyage has no usage API. The plan is learned from the first requests a run makes."
          : "Detected from Voyage's own rate-limit responses."
      }
    />
  );
}

/**
 * The free pixel grant as a meter: what has been spent, what this run would
 * add, and the spend guard as a mark on the same track. Figures are Oculus's
 * own count (see `VoyageUsage`).
 */
function AllowanceMeter({
  usage,
  runPixels,
  onChange,
}: {
  usage: VoyageUsage;
  /** This run's projection, drawn ahead of the fill. 0 when unknown. */
  runPixels: number;
  onChange: (percent: number) => void;
}) {
  const grant = Math.max(1, usage.free_pixels);
  const spent = (usage.pixels / grant) * 100;
  const projected = Math.min((runPixels / grant) * 100, Math.max(0, 100 - spent));

  return (
    <div className="py-2">
      <div className="flex items-baseline justify-between gap-4">
        <p className="flex items-center gap-1 text-xs text-foreground">
          Free allowance
          <Tooltip>
            <TooltipTrigger asChild>
              <span className="text-muted-foreground" aria-label="Where this number comes from">
                <Info size={12} weight="bold" />
              </span>
            </TooltipTrigger>
            <TooltipContent>
              No usage API — Oculus&rsquo;s own count of what it sent
            </TooltipContent>
          </Tooltip>
        </p>
        <p className="text-xs text-foreground tabular-nums">
          {si(usage.pixels)} of {si(usage.free_pixels)} pixels ·{" "}
          {spent < 0.1 && usage.pixels > 0 ? "<0.1" : spent.toFixed(1)}%
        </p>
      </div>

      <div className="relative mt-2 h-2.5 overflow-hidden rounded-full bg-surface">
        <div className="absolute inset-0 flex">
          <div
            className="h-full bg-chart-1"
            style={{ width: `${Math.min(spent, 100)}%`, minWidth: usage.pixels > 0 ? 3 : 0 }}
          />
          {projected > 0 ? (
            <div className="h-full bg-chart-1/35" style={{ width: `${projected}%`, minWidth: 3 }} />
          ) : null}
        </div>
        {usage.stop_at_percent > 0 && usage.stop_at_percent < 100 ? (
          <div
            className="absolute top-0 h-full w-[2px] bg-destructive"
            style={{ left: `${usage.stop_at_percent}%` }}
          />
        ) : null}
      </div>

      <div className="mt-2 flex items-center justify-between gap-4">
        <p className="text-[11px] text-muted-foreground">
          {runPixels > 0 ? `This run adds ${si(runPixels)}. ` : ""}
          {usage.stop_at_percent === 0
            ? "No limit — Voyage charges past 100%."
            : usage.stop_at_percent === 100
              ? "Stops before Voyage starts charging."
              : "Stops at the mark."}
        </p>
        <Select
          value={String(usage.stop_at_percent)}
          onValueChange={(value) => onChange(Number(value))}
        >
          <SelectTrigger aria-label="Stop indexing at" size="sm" className="h-7 w-32 text-xs">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            {[50, 75, 90, 100].map((option) => (
              <SelectItem key={option} value={String(option)} className="text-xs">
                Stop at {option}%
              </SelectItem>
            ))}
            <SelectItem value="0" className="text-xs">
              No limit
            </SelectItem>
          </SelectContent>
        </Select>
      </div>
    </div>
  );
}

/** Colour follows the file kind, never its current rank. */
const KIND_COLOR: Record<string, string> = {
  pdf: "bg-chart-1",
  docx: "bg-chart-2",
  pptx: "bg-chart-3",
  doc: "bg-chart-4",
  ppt: "bg-chart-5",
};

/**
 * What pressing Index will cost, in time and dollars — measured by Rust
 * (`app/src-tauri/src/embed/estimate.rs`). The free pixel grant applies to
 * every account, so a payment method buys speed, not price: when tier 1 is
 * faster the headline is the time saved.
 */
function RunEstimate({
  estimate,
  estimating,
  usage,
}: {
  estimate: EmbedEstimate | null;
  estimating: boolean;
  usage: VoyageUsage | null;
}) {
  if (estimating && !estimate) {
    return (
      <p className="pt-2 text-[11px] text-muted-foreground">Measuring what is outstanding…</p>
    );
  }
  if (!estimate || estimate.files === 0 || estimate.pages === 0) return null;

  // Never pitch an upgrade off an `assumed` tier.
  const upgrade =
    estimate.tier_free &&
    estimate.tier_source !== "assumed" &&
    estimate.seconds_tier1 < estimate.seconds / 2;
  const cut = estimate.stops_after_pages;
  const kinds = estimate.kinds.filter((bucket) => bucket.pages > 0);
  const totalPages = Math.max(1, estimate.pages);

  return (
    <Alert
      variant={cut != null || estimate.cost_usd > 0 ? "warning" : "default"}
      className="mt-3"
    >
      <AlertTitle className="text-xs">
        {upgrade
          ? `Save ${roughly(estimate.seconds).replace(/^about /, "")} of indexing, at no extra cost`
          : `${roughly(estimate.seconds)} to index ${estimate.pages.toLocaleString()} pages${
              estimate.tier_source === "assumed" ? ", if this account is on tier 1" : ""
            }`}
      </AlertTitle>
      <AlertDescription className="gap-2 text-[11px]">
        {upgrade ? (
          <div className="flex w-full items-center gap-3">
            <span className="tabular-nums">
              {roughly(estimate.seconds)} now · {roughly(estimate.seconds_tier1)} on tier 1
            </span>
            <button
              type="button"
              className="inline-flex items-center gap-1 text-brand hover:underline"
              onClick={() => void openUrl(VOYAGE_DASHBOARD)}
            >
              Add a payment method
              <ArrowSquareOut size={11} weight="bold" />
            </button>
          </div>
        ) : null}

        <div className="w-full">
          <div className="flex h-1.5 gap-[2px] overflow-hidden rounded-full bg-surface">
            {kinds.map((bucket) => (
              <div
                key={bucket.label}
                className={cn("h-full", KIND_COLOR[bucket.label] ?? "bg-chart-other")}
                style={{ width: `${(bucket.pages / totalPages) * 100}%`, minWidth: 4 }}
              />
            ))}
          </div>
          <div className="mt-1.5 flex flex-wrap gap-x-4 gap-y-1">
            {kinds.map((bucket) => (
              <span key={bucket.label} className="flex items-center gap-1.5 tabular-nums">
                <span
                  className={cn(
                    "size-2 shrink-0 rounded-[3px]",
                    KIND_COLOR[bucket.label] ?? "bg-chart-other",
                  )}
                />
                {bucket.label.toUpperCase()} {bucket.files}
                <span className="text-muted-foreground">
                  {bucket.pages.toLocaleString()} pages
                </span>
              </span>
            ))}
          </div>
        </div>

        <p className="tabular-nums">
          Estimated cost{" "}
          <span className="text-foreground">
            {estimate.cost_usd > 0 ? `$${estimate.cost_usd.toFixed(2)}` : "Free"}
          </span>{" "}
          · {si(estimate.pixels)} of {si(usage?.free_pixels ?? 150e9)} pixels (
          {((estimate.pixels / Math.max(1, usage?.free_pixels ?? 150e9)) * 100).toFixed(1)}%)
        </p>

        {cut != null ? (
          <p className="font-medium">
            Stops after {cut.toLocaleString()} of {estimate.pages.toLocaleString()} pages at
            the spend limit above.
          </p>
        ) : null}

        {estimate.unreadable > 0 ? (
          <p className="font-medium">
            {estimate.unreadable} file{estimate.unreadable === 1 ? "" : "s"} could not be
            measured and may fail.
          </p>
        ) : null}
      </AlertDescription>
    </Alert>
  );
}

/**
 * Files done plus the fraction of the current document, over the queue as it
 * stands now (it can grow mid-run). The page term keeps a slow run visibly
 * moving.
 */
function runPercent(progress: IndexProgress): number {
  if (progress.total <= 0) return 0;
  const inside =
    progress.totalPages > 0 ? Math.min(progress.pagesDone / progress.totalPages, 1) : 0;
  return Math.min(((progress.done + inside) / progress.total) * 100, 100);
}

/** The same in words; pages only once the document has reported some. */
function runLabel(progress: IndexProgress): string {
  const files = `${progress.done} of ${progress.total}`;
  if (progress.totalPages > 0) {
    return `${files} · page ${progress.pagesDone} of ${progress.totalPages}`;
  }
  return files;
}

/**
 * Start, watch and stop an index run. Names the current file so a slow run
 * shows it is alive (see docs/retrieval.md: an embed blocks for minutes).
 */
function IndexRunRow({
  outstanding,
  ready,
  run,
}: {
  outstanding: number | null;
  ready: boolean;
  run: IndexState;
}) {
  const nothingToDo = outstanding === 0;

  return (
    <div className="pt-2">
      <div className="flex items-center justify-between gap-4">
        <div className="min-w-0">
          <p className="text-xs text-foreground">Build the index</p>
          <p className="text-[11px] text-muted-foreground">
            {run.running
              ? run.progress
                ? `${runLabel(run.progress)}${
                    run.progress.filename ? ` · ${run.progress.filename}` : ""
                  }`
                : "Working out what is outstanding…"
              : outstanding == null
                ? "Embeds every parsed PDF that is not in the current space."
                : nothingToDo
                  ? "Every parsed PDF is in the current space."
                  : `${outstanding} file${outstanding === 1 ? "" : "s"} to embed.`}
          </p>
        </div>
        {run.running ? (
          <Button variant="outline" size="xs" disabled={run.stopping} onClick={() => run.stop()}>
            {/* Stopping lands on a file boundary. */}
            {run.stopping ? "Stopping…" : "Stop"}
          </Button>
        ) : (
          <Button
            size="xs"
            disabled={!ready || nothingToDo}
            onClick={() => void run.start()}
          >
            Index
          </Button>
        )}
      </div>

      {run.running && run.progress ? (
        <Progress value={runPercent(run.progress)} className="mt-2 h-1" />
      ) : null}

      {!ready && !run.running ? (
        <p className="mt-2 text-[11px] text-muted-foreground">
          Save a Voyage API key first — there is nothing to embed against without one.
        </p>
      ) : null}

      {run.result ? (
        <p className="mt-2 text-[11px] leading-relaxed text-muted-foreground">
          {run.result.stopped ? "Stopped after " : "Indexed "}
          {run.result.files} file{run.result.files === 1 ? "" : "s"} ·{" "}
          {run.result.pages.toLocaleString()} pages
          {run.result.errors.length
            ? ` · ${run.result.errors.length} failed`
            : ""}
        </p>
      ) : null}

      {/* Named, not counted: reasons differ per file. */}
      {run.result?.errors.length ? (
        <ul className="mt-1 space-y-0.5">
          {run.result.errors.slice(0, 5).map((message) => (
            <li key={message} className="text-[11px] leading-relaxed text-destructive">
              {message}
            </li>
          ))}
          {run.result.errors.length > 5 ? (
            <li className="text-[11px] text-muted-foreground">
              …and {run.result.errors.length - 5} more
            </li>
          ) : null}
        </ul>
      ) : null}

      {run.error ? (
        <p className="mt-2 text-[11px] leading-relaxed text-destructive">{run.error}</p>
      ) : null}
    </div>
  );
}
