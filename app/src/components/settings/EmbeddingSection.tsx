import { useEffect, useState } from "react";
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
import { getUnembeddedPdfs } from "@/lib/retrieval";
import { useIndexStore, type IndexState } from "@/stores/indexStore";
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
}

/**
 * The embedding backend, and the index it owns.
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

  const run = useIndexStore();

  const reload = () => {
    invoke<EmbedSettings>("embed_settings")
      .then(setSettings)
      .catch((cause) => {
        console.error("embed settings failed", cause);
        setError("Could not read the embedding settings.");
      });
    getUnembeddedPdfs()
      .then((files) => setOutstanding(files.length))
      .catch(() => setOutstanding(null));
  };

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
    return () => {
      cancelled = true;
    };
  }, []);

  // A finished run moves every number on this page, so re-read them rather
  // than leaving counts that were true before it started.
  useEffect(() => {
    if (!run.running && (run.result || run.error)) reload();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [run.running]);

  const selected = settings?.engines.find((engine) => engine.id === settings.engine) ?? null;
  const unavailable = settings?.engines.filter((engine) => !engine.available) ?? [];

  const apply = async (engine: string) => {
    setSwitching(true);
    setError(null);
    try {
      setSettings(await invoke<EmbedSettings>("embed_set_engine", { engine }));
      setPrompt(null);
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
    } catch (cause) {
      console.error("Voyage key removal failed", cause);
      setKeyNote({ kind: "error", text: String(cause) });
    }
  };

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

        {/* The architecture, said out loud. The local engine is a real arm of
            the seam and a real value of this setting; it has no server to talk
            to yet, and saying so beats hiding the option or shipping a live
            control that points at a closed port. */}
        {unavailable.map((engine) => (
          <p key={engine.id} className="text-[11px] leading-relaxed text-muted-foreground">
            {engine.label}: {engine.unavailable_reason}
          </p>
        ))}

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
            the library held Voyage vectors while every one of them was Qwen.
            The stale counts below are where the stored side is told. */}
        <StatRow
          label="Search space"
          value={
            settings?.index.model && settings.index.dim
              ? `${settings.index.model} · ${settings.index.dim}d`
              : "—"
          }
        />
        {settings && settings.index.pages_stale > 0 ? (
          <StatRow
            label="Awaiting re-index"
            value={`${settings.index.pages_stale.toLocaleString()} pages`}
          />
        ) : null}

        {/* The one place the gap is spelled out rather than left to be
            inferred from two numbers that happen to disagree. */}
        {settings && settings.index.pages_stale > 0 ? (
          <p className="pt-1 text-[11px] leading-relaxed text-muted-foreground">
            {settings.index.pages_stale.toLocaleString()} stored pages were embedded by{" "}
            {settings.index.stale_models.length
              ? settings.index.stale_models.join(", ")
              : "a retired model"}
            . They cannot be compared against a query from {settings.index.model}, so they are
            not searchable until they are rebuilt. Nothing migrates between the two spaces.
          </p>
        ) : null}

        {/* The run. Until this existed the index could only be built from a
            terminal, which meant a library could sit permanently unsearchable
            with nothing in the app admitting it or offering a fix. */}
        <IndexRunRow
          outstanding={outstanding}
          ready={Boolean(settings?.credentials_ready)}
          run={run}
        />

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
                ? `${run.progress.done} of ${run.progress.total}${
                    run.progress.filename ? ` · ${run.progress.filename}` : ""
                  }`
                : "Working out what is outstanding…"
              : outstanding == null
                ? "Embeds every parsed PDF that is not in the current space."
                : nothingToDo
                  ? "Every parsed PDF is in the current space."
                  : `${outstanding} file${outstanding === 1 ? "" : "s"} to embed. This can take hours on a free Voyage account.`}
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
