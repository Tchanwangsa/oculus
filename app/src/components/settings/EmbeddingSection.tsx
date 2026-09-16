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
    return () => {
      cancelled = true;
    };
  }, []);

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
        {/* What the stored vectors *are*, not what this build would write —
            the two disagree for exactly as long as a re-index is outstanding,
            and that gap is the thing worth seeing. */}
        <StatRow
          label="Vector space"
          value={
            settings?.index.model && settings.index.dim
              ? `${settings.index.model} · ${settings.index.dim}d`
              : "Empty"
          }
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
