import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";
import { ArrowSquareOut, Info } from "@phosphor-icons/react";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { cn } from "@/lib/utils";
import { embedReady, getUnembeddedPdfs } from "@/lib/retrieval";
import { Progress } from "@/components/ui/progress";
import { useIndexStore, type IndexProgress, type IndexState } from "@/stores/indexStore";
import { Section, StatRow } from "@/pages/settings/section";
import { ReindexConfirmDialog, type ReindexPrompt } from "./ReindexConfirmDialog";

/** Mirrors `EngineOption` in `app/src-tauri/src/embed/commands.rs`. */
interface EngineOption {
  id: string;
  label: string;
  detail: string;
  available: boolean;
  /** Present only when `available` is false, and then always. */
  unavailable_reason: string | null;
}

/**
 * Mirrors `VoyageUsage` in `app/src-tauri/src/embed/commands.rs`.
 *
 * **These are Oculus's numbers, not Voyage's.** Voyage publishes no usage
 * endpoint, so everything here is `voyage-usage.json` — what this app reserved
 * before each request, topped up against what each response said it was
 * billed. The page says so where it shows them, because a figure that looks
 * like an account balance and is really a local tally would be read as one.
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

// ── Numbers, in the units a sentence can carry ───────────────────────────────

/** 5,180,000,000 → "5.2B". Pixels and tokens run to ten digits and a settings
 *  row is not where anyone counts them. */
function si(value: number): string {
  if (value >= 1e9) return `${(value / 1e9).toFixed(1)}B`;
  if (value >= 1e6) return `${(value / 1e6).toFixed(1)}M`;
  if (value >= 1e3) return `${Math.round(value / 1e3)}K`;
  return value.toLocaleString();
}

/** A duration for a sentence, not a stopwatch: this is a prediction about a run
 *  that may take most of a day, and "17h 43m" claims a precision the estimate
 *  does not have. */
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
 * One model is selected and search runs against that one — no fallback
 * between engines, no fusing two spaces at query time. Which is why the
 * control is not a plain `onValueChange`: changing it throws every stored
 * vector away, so the change is announced first (`ReindexConfirmDialog`) and
 * only then handed to Rust, which clears the index and writes the setting in
 * one call.
 *
 * The engine list, the labels and the reason an engine is unavailable all come
 * from Rust, so the page cannot offer something the backend would refuse, or
 * explain a refusal in different words.
 *
 * **The stats are a short list on purpose.** This section used to spell the
 * stale-vector story out in a row *and* a paragraph, on a page where the one
 * thing a student actually has to decide is whether to press Index. What is
 * left answers that: what is in the index, what is not, what the account is,
 * and — in `RunEstimate` — what pressing it will cost in hours and in dollars.
 */
export function EmbeddingSection() {
  const [settings, setSettings] = useState<EmbedSettings | null>(null);
  const [prompt, setPrompt] = useState<(ReindexPrompt & { engine: string }) | null>(null);
  const [switching, setSwitching] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const [key, setKey] = useState("");
  const [keyNote, setKeyNote] = useState<{ kind: "error" | "warn"; text: string } | null>(null);
  const [checkingKey, setCheckingKey] = useState(false);

  // How many files a run would actually touch. Not derivable from the counts
  // above — `pages_stale` counts pages from any model, and a file can be
  // partly embedded — so it is the same query the run itself walks.
  const [outstanding, setOutstanding] = useState<number | null>(null);

  // Kept apart from `settings` because it is *slow*: Rust opens every
  // outstanding PDF to measure its pages. The rest of the section draws while
  // this is still running, and `null` renders as "Measuring…" rather than as
  // an empty banner.
  const [estimate, setEstimate] = useState<EmbedEstimate | null>(null);
  const [estimating, setEstimating] = useState(false);

  const run = useIndexStore();

  // **One sweep at a time, and never two at once.** `embed_estimate` opens
  // every outstanding PDF, and pdfium is a single session process-wide
  // (`raster.rs`), so a second concurrent call does not run twice as fast — it
  // queues behind the first for several seconds. React's StrictMode fires the
  // mount effect twice in dev, which is how this page came to ask for two
  // library-wide sweeps every time it opened.
  //
  // A request that arrives while one is running is *remembered*, not dropped:
  // changing the spend limit re-measures, and silently keeping the old answer
  // would leave the banner quoting a cut-off that is no longer the setting.
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

  // A finished run moves every number on this page, so re-read them rather
  // than leaving counts that were true before it started. The estimate
  // especially: a run is the only thing that teaches the ledger what tier this
  // account is on, so the hours quoted here are often wrong until one has run.
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

  // An empty index has nothing to lose, so the dialog would be ceremony — and
  // a confirmation raised over nothing is how people learn to click through
  // the one that matters.
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

  // The guard moves where the run stops, so the estimate is re-measured with
  // it — that is the only way the banner's "would stop after N pages" can be
  // true of the setting that is actually in force.
  const setBudget = async (percent: number) => {
    try {
      setSettings(await invoke<EmbedSettings>("embed_set_budget", { percent }));
      loadEstimate();
    } catch (cause) {
      console.error("embed budget change failed", cause);
      setError(String(cause));
    }
  };

  // Rust checks the key against Voyage before it reaches the keychain, so a
  // typo is named here rather than at the next page indexed.
  const saveKey = async () => {
    if (!key.trim()) return;
    setCheckingKey(true);
    setKeyNote(null);
    try {
      const verdict = await invoke<string>("voyage_set_api_key", { key: key.trim() });
      setKey("");
      setSettings((prev) => (prev ? { ...prev, credentials_ready: true } : prev));
      // The third pipeline stage and auto-embed both hang off this: a key is
      // what makes the app willing to queue work at all. Re-asked rather than
      // assumed, because the *engine* has a say too.
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
      // And off again — nothing queues itself against a backend with no key.
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
          <Select
            value={settings?.engine ?? ""}
            disabled={!settings || switching}
            onValueChange={choose}
          >
            <SelectTrigger aria-label="Embedding model" size="sm" className="h-7 w-48 text-xs">
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

        {/* The unavailable engines used to explain themselves in a paragraph
            here as well as in the menu. The menu already says "Unavailable" on
            the row nobody can pick, and Rust still refuses the engine in the
            same words if a stale UI asks for it — so the paragraph was a
            sentence about a control that is not in use, on a page whose job is
            the one that is. `unavailable_reason` is still the source of both. */}

        {settings && !settings.credentials_ready && settings.engine === "cloud" ? (
          <p className="text-[11px] leading-relaxed text-warning">
            Nothing can be indexed or searched until a Voyage API key is saved below.
          </p>
        ) : null}

        <div className="py-2">
          <div className="flex items-center justify-between gap-4">
            <div>
              <p className="text-xs text-foreground">Voyage API key</p>
              <p className="text-[11px] text-muted-foreground">
                Stored in your Mac keychain, never in the library database.
              </p>
            </div>
            {settings?.credentials_ready ? (
              <div className="flex items-center gap-2">
                <span className="text-xs text-success">Connected</span>
                <Button variant="outline" size="xs" onClick={() => void deleteKey()}>
                  Remove
                </Button>
              </div>
            ) : (
              <div className="flex items-center gap-2">
                <Input
                  aria-label="Voyage API key"
                  type="password"
                  autoComplete="off"
                  value={key}
                  onChange={(event) => setKey(event.target.value)}
                  onKeyDown={(event) => {
                    if (event.key === "Enter") void saveKey();
                  }}
                  placeholder="Paste key"
                  className="h-7 w-44 text-xs"
                />
                <Button
                  size="xs"
                  disabled={!key.trim() || checkingKey}
                  onClick={() => void saveKey()}
                >
                  {checkingKey ? "Checking…" : "Save"}
                </Button>
              </div>
            )}
          </div>
          {keyNote ? (
            <p
              className={cn(
                "mt-2 text-[11px] leading-relaxed",
                keyNote.kind === "error" ? "text-destructive" : "text-warning",
              )}
            >
              {keyNote.text}
            </p>
          ) : null}
        </div>

        <StatRow
          label="Pages indexed"
          value={settings ? settings.index.pages_embedded.toLocaleString() : "—"}
        />
        <StatRow
          label="Files indexed"
          value={settings ? settings.index.files_embedded.toLocaleString() : "—"}
        />
        {/* `index.model` is the space this build *writes*, not the space the
            stored vectors are in — `stats` reads it off the seam's constants so
            it can answer before a key exists. Labelling it "Vector space" said
            the library held Voyage vectors while every one of them was Qwen. */}
        <StatRow
          label="Search space"
          value={
            settings?.index.model && settings.index.dim
              ? `${settings.index.model} · ${settings.index.dim}d`
              : "—"
          }
        />
        {/* The one row about what is *missing*, and it replaced two that were
            about stale vectors. It counts files rather than pages because a
            file is what the run walks and what a failure is scoped to — and it
            follows the current space, so vectors from a retired model read as
            not indexed, which is exactly what they are. */}
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

        {/* The run. Until this existed the index could only be built from a
            terminal, which meant a library could sit permanently unsearchable
            with nothing in the app admitting it or offering a fix. */}
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
 * What programme this account is on.
 *
 * Allowed to say it does not know, because Voyage has no endpoint that
 * answers: the per-minute limits are *detected* from 429 bodies during a run
 * (`embed/voyage/ledger.rs`), so an install that has never indexed anything has
 * never had the chance to find out, and "Free" printed over that opening guess
 * would be a claim about somebody's billing that nothing checked.
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
 * The free pixel grant as a meter, with the spend guard drawn on it.
 *
 * Three facts in one object, which is why they stopped being three rows and a
 * paragraph: what has been spent, what this run would add, and where indexing
 * stops. The guard is a number you set against a number you cannot see, so
 * showing it as a line across the same track is the whole point — "100%" means
 * nothing until it is the mark the fill is heading for.
 *
 * Every figure is **Oculus's own count**: Voyage publishes no usage endpoint,
 * so this is `voyage-usage.json`, what the client reserved before each request
 * and topped up from what each response said it was billed. That caveat is a
 * tooltip rather than a sentence, because it qualifies the number without ever
 * being the thing you came to read.
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

      {/* Same meter grammar as Settings → Storage: a stacked fill on a track,
          with the limit as a `destructive` hairline across it. */}
      <div className="relative mt-2 h-2.5 overflow-hidden rounded-full bg-surface">
        <div className="absolute inset-0 flex">
          <div
            className="h-full bg-chart-1"
            style={{ width: `${Math.min(spent, 100)}%`, minWidth: usage.pixels > 0 ? 3 : 0 }}
          />
          {/* This run, ahead of what is already spent — the reason the guard is
              worth setting before pressing Index rather than after. */}
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
            {/* Past the grant is a price, not a wall. Somebody who means to pay
                for it turns the guard off and the app stops having an opinion. */}
            <SelectItem value="0" className="text-xs">
              No limit
            </SelectItem>
          </SelectContent>
        </Select>
      </div>
    </div>
  );
}

/** Fixed per file type, the way Settings → Storage fixes them per entity:
 *  colour follows the kind, never its current rank. */
const KIND_COLOR: Record<string, string> = {
  pdf: "bg-chart-1",
  docx: "bg-chart-2",
  pptx: "bg-chart-3",
  doc: "bg-chart-4",
  ppt: "bg-chart-5",
};

/**
 * What pressing Index will actually cost, in hours and in dollars.
 *
 * Every number is measured rather than phrased: Rust reads each outstanding
 * PDF's page boxes, bills them the way Voyage does (pixels, capped at 2,000,000
 * an image) and packs them with the same batcher the run uses. See
 * `app/src-tauri/src/embed/estimate.rs`.
 *
 * **The headline is the saving, not the total**, and the reason is the one
 * genuinely counter-intuitive fact about this API: the 150B free pixels are
 * granted to *every* account, so a payment method does not make a coursework
 * library cheaper — it is already free — it makes it two hundred times faster.
 * "About 18 hours" is a number to sigh at; "save about 18 hours, at no extra
 * cost" is a number to act on, and it is the same measurement.
 *
 * An earlier draft said all of that in five paragraphs, including a sentence
 * about page orientation and one about the billing cap. They were true and
 * nobody would read them. What is left is a headline, a comparison, a
 * breakdown and a price.
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

  // Only when tier 1 is a real, measured improvement — never over an `assumed`
  // tier, which would be pitching an upgrade off a guess.
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

        {/* The breakdown, in Storage's grammar: one stacked bar, then a row per
            kind. It replaced a sentence that counted page orientations. */}
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

        {/* Only when the guard would actually bite — the meter above already
            says what it does, and repeating that over a run it would not touch
            is the kind of line people stop reading the banner over. */}
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
 * What the run has finished, as a fraction of what it holds — files, plus the
 * part of the current document that is done.
 *
 * The page term is what makes it move. Files alone step once an hour on a
 * 200-page deck at the free programme's ~2.8 pages a minute, and a bar that
 * has not moved in an hour is indistinguishable from a hang. The queue can
 * also *grow* mid-run (a sync finishing a parse appends a file), so this is
 * deliberately a fraction of the total as it stands now rather than a promise
 * about the end.
 */
function runPercent(progress: IndexProgress): number {
  if (progress.total <= 0) return 0;
  const inside =
    progress.totalPages > 0 ? Math.min(progress.pagesDone / progress.totalPages, 1) : 0;
  return Math.min(((progress.done + inside) / progress.total) * 100, 100);
}

/** The same in words. Pages only once the document has reported some — a
 *  denominator of zero would read as a finished file. */
function runLabel(progress: IndexProgress): string {
  const files = `${progress.done} of ${progress.total}`;
  if (progress.totalPages > 0) {
    return `${files} · page ${progress.pagesDone} of ${progress.totalPages}`;
  }
  return files;
}

/**
 * Start, watch and stop an index run.
 *
 * It names the file it is on, not just a percentage, because on a Voyage
 * account with no payment method this is ~2.8 pages a minute — a run that can
 * take most of a day, where a bar that has not moved in ten minutes is
 * indistinguishable from a hang and a filename that changed is proof of life.
 *
 * No toast and no bottom bar, per the house rules: the page that owns the
 * index shows the detail, and the sidebar carries it once you navigate away.
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
            {/* Stopping lands on a file boundary, so the button says so
                rather than pretending the click was instant. */}
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

      {/* One bar for the whole run, and the page inside the current document is
          part of its fraction rather than a second bar: a 200-page deck on the
          free programme is an hour in which a file-counting bar does not move
          at all, which is the complaint this exists to answer. */}
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

      {/* Named, not counted: a run that failed on three files should say which,
          because the reasons differ per file and one of them may be the whole
          account's. */}
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
