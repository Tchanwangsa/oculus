import { useEffect, useRef, useState } from "react";
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
import { Separator } from "@/components/ui/separator";
import {
  DEFAULT_PARSE_SETTINGS,
  getParseSettings,
  getPdfPipelineRows,
  setParseSettings,
  type ParseBackend,
  type ParseSettings,
} from "@/lib/db";
import { cn } from "@/lib/utils";
import { EmbeddingSection } from "@/components/settings/EmbeddingSection";
import { Section, StatRow } from "./section";

interface SidecarHealth {
  status: string;
  pid: number;
  parser_version: number;
  quality_current: string | null;
  quality_queued: number;
  cloud_current: string[];
  memory: {
    footprint_mb: number;
    cap_mb: number;
    peak_mb: number;
    kills: number;
    measurement_complete: boolean;
  };
  parse: {
    backend: ParseBackend;
    cloud_usage: {
      files: number;
      pages: number;
      daily_file_limit: number;
      daily_priority_page_limit: number;
      priority_degraded: boolean;
    } | null;
    /** A parse hit 401 with the stored token. Latched until a token is saved. */
    cloud_token_rejected: boolean;
  };
}

interface LibraryCounts {
  tracked: number;
  parsed: number;
}

type SaveState = "idle" | "saving" | "saved" | "error";

const BACKEND_LABELS: Record<ParseBackend, string> = {
  local: "Local only",
  cloud: "MinerU cloud",
  auto: "Automatic (cloud first)",
};

export default function SettingsLibraryPage() {
  const [library, setLibrary] = useState<LibraryCounts | null>(null);
  const [sidecar, setSidecar] = useState<SidecarHealth | null>(null);
  const [sidecarDown, setSidecarDown] = useState(false);
  const [settings, setSettings] = useState<ParseSettings>(DEFAULT_PARSE_SETTINGS);
  const [memoryGb, setMemoryGb] = useState(DEFAULT_PARSE_SETTINGS.memoryCapMb / 1024);
  const [hasToken, setHasToken] = useState(false);
  const [token, setToken] = useState("");
  const [tokenNote, setTokenNote] = useState<{ kind: "error" | "warn"; text: string } | null>(null);
  const [checkingToken, setCheckingToken] = useState(false);
  const [saveState, setSaveState] = useState<SaveState>("idle");
  const saveVersion = useRef(0);
  const latestSettings = useRef(DEFAULT_PARSE_SETTINGS);
  const pendingSave = useRef<Promise<void>>(Promise.resolve());

  useEffect(() => {
    let cancelled = false;
    Promise.all([
      getPdfPipelineRows(),
      getParseSettings(),
      invoke<boolean>("mineru_has_api_key"),
    ])
      .then(([rows, parse, tokenPresent]) => {
        if (cancelled) return;
        setLibrary({
          tracked: rows.length,
          parsed: rows.filter((row) => row.parse_status === "quality").length,
        });
        setSettings(parse);
        latestSettings.current = parse;
        setMemoryGb(parse.memoryCapMb / 1024);
        setHasToken(tokenPresent);
      })
      .catch((error) => console.error("library settings failed", error));
    return () => { cancelled = true; };
  }, []);

  useEffect(() => {
    let cancelled = false;
    const poll = async () => {
      try {
        const health = await invoke<SidecarHealth>("sidecar_health");
        if (!cancelled) {
          setSidecar(health);
          setSidecarDown(false);
        }
      } catch {
        if (!cancelled) setSidecarDown(true);
      }
    };
    poll();
    const id = setInterval(poll, 5000);
    return () => {
      cancelled = true;
      clearInterval(id);
    };
  }, []);

  const persist = async (next: ParseSettings) => {
    const version = ++saveVersion.current;
    setSettings(next);
    latestSettings.current = next;
    setSaveState("saving");
    try {
      // Keep rapid edits ordered across both SQLite and the live sidecar.
      const save = pendingSave.current.catch(() => {}).then(async () => {
        await setParseSettings(next);
        await invoke("sidecar_set_limits", {
          memoryCapMb: next.memoryCapMb, backend: next.backend,
        });
      });
      pendingSave.current = save;
      await save;
      if (version === saveVersion.current) setSaveState("saved");
    } catch (error) {
      console.error("parse settings save failed", error);
      if (version === saveVersion.current) setSaveState("error");
    }
  };

  const commitMemory = () => {
    const gigabytes = Math.max(5, Math.round(Number.isFinite(memoryGb) ? memoryGb : 5));
    setMemoryGb(gigabytes);
    if (gigabytes * 1024 !== latestSettings.current.memoryCapMb) {
      void persist({ ...latestSettings.current, memoryCapMb: gigabytes * 1024 });
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
      setSidecar((prev) =>
        prev ? { ...prev, parse: { ...prev.parse, cloud_token_rejected: false } } : prev,
      );
      setTokenNote(
        verdict === "unverified"
          ? { kind: "warn", text: "Saved, but MinerU was unreachable — it has not been checked." }
          : null,
      );
    } catch (error) {
      setTokenNote({ kind: "error", text: String(error) });
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
      setSidecar((prev) =>
        prev ? { ...prev, parse: { ...prev.parse, cloud_token_rejected: false } } : prev,
      );
    } catch (error) {
      console.error("MinerU token removal failed", error);
      setTokenNote({ kind: "error", text: String(error) });
    }
  };

  // Only the sidecar sees a mid-parse 401, and it reports it in the health the
  // page already polls — no separate check on MinerU.
  const tokenExpired = hasToken && Boolean(sidecar?.parse.cloud_token_rejected);

  const activeLabel = sidecar?.quality_current
    ? `Parsing ${sidecar.quality_current}`
    : sidecar?.cloud_current.length
      ? `${sidecar.cloud_current.length} cloud parse${sidecar.cloud_current.length === 1 ? "" : "s"}`
      : "Idle";

  return (
    <>
      <Section title="Library" description="Where synced PDFs are in the parse pipeline.">
        <div>
          <StatRow label="PDFs tracked" value={library ? String(library.tracked) : "—"} />
          <StatRow
            label="Quality parsed"
            value={library ? `${library.parsed}/${library.tracked}` : "—"}
          />
        </div>
      </Section>

      <Separator className="my-7" />

      <Section
        title="PDF processing"
        description="Choose where quality parsing runs and bound the memory used by all local parser processes together."
      >
        <div className="space-y-1">
          <div className="flex items-center justify-between py-2">
            <div>
              <p className="text-xs text-foreground">Quality parser</p>
              <p className="text-[11px] text-muted-foreground">
                Cloud processing is opt-in and uses MinerU’s service in China.
              </p>
            </div>
            <Select
              value={settings.backend}
              onValueChange={(value) =>
                void persist({ ...latestSettings.current, backend: value as ParseBackend })
              }
            >
              <SelectTrigger aria-label="Quality parser backend" size="sm" className="h-7 w-48 text-xs">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {(Object.keys(BACKEND_LABELS) as ParseBackend[]).map((backend) => (
                  <SelectItem key={backend} value={backend} className="text-xs">
                    {BACKEND_LABELS[backend]}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>

          <div className="flex items-center justify-between py-2">
            <div>
              <p className="text-xs text-foreground">Local memory cap</p>
              <p className="text-[11px] text-muted-foreground">
                8 GB recommended. Lower caps can prevent quality parsing or indexing.
              </p>
            </div>
            <div className="flex items-center gap-2">
              <Input
                aria-label="Local memory cap in gigabytes"
                type="number"
                min={5}
                step={1}
                value={memoryGb}
                onChange={(event) => setMemoryGb(Number(event.target.value))}
                onBlur={commitMemory}
                onKeyDown={(event) => {
                  if (event.key === "Enter") event.currentTarget.blur();
                }}
                className="h-7 w-20 text-xs text-right tabular-nums"
              />
              <span className="w-5 text-xs text-muted-foreground">GB</span>
            </div>
          </div>

          <div className="py-2">
            <div className="flex items-center justify-between gap-4">
              <div>
                <p className="text-xs text-foreground">MinerU API token</p>
                <p className="text-[11px] text-muted-foreground">
                  Stored in your Mac keychain, never in the library database.
                </p>
              </div>
              {hasToken && !tokenExpired ? (
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
                MinerU refused this token during a parse. Cloud parsing is paused and files are
                being parsed locally until you paste a new one.
              </p>
            ) : null}
            {settings.backend !== "local" ? (
              <p className="mt-2 text-[11px] leading-relaxed text-muted-foreground">
                Lecture PDFs are uploaded to MinerU and its PRC-hosted OSS storage. Results may be
                cached by MinerU (its documented default cache tolerance is 15 minutes, not a deletion guarantee).
                Use Local only to keep course material on this Mac.
              </p>
            ) : null}
            {settings.backend !== "local" && !hasToken && !tokenExpired ? (
              <p className="mt-1.5 text-[11px] text-warning">
                Add a token before cloud parsing can run; until then Oculus uses the local parser.
              </p>
            ) : null}
          </div>

          <p
            aria-live="polite"
            className={cn(
              "min-h-4 pt-1 text-[11px]",
              saveState === "error" ? "text-destructive" : "text-muted-foreground",
            )}
          >
            {saveState === "saving"
              ? "Saving…"
              : saveState === "saved"
                ? "Saved"
                : saveState === "error"
                  ? "Could not save these settings"
                  : ""}
          </p>
        </div>
      </Section>

      <Separator className="my-7" />

      <EmbeddingSection />

      <Separator className="my-7" />

      <Section title="Sidecar" description="The local service that parses and embeds PDFs.">
        <div>
          <div className="flex items-center justify-between py-2">
            <span className="text-xs text-muted-foreground">Status</span>
            <span className="flex items-center gap-1.5 text-xs text-foreground">
              {sidecarDown ? "Not responding" : sidecar ? activeLabel : "Checking…"}
              <span
                className={cn(
                  "size-1.5 rounded-full",
                  sidecarDown
                    ? "bg-destructive"
                    : sidecar?.quality_current || sidecar?.cloud_current.length
                      ? "bg-brand animate-pulse"
                      : sidecar
                        ? "bg-success"
                        : "bg-muted-foreground/40",
                )}
              />
            </span>
          </div>
          {!sidecarDown && sidecar ? (
            <>
              <StatRow
                label="Memory"
                value={`${(sidecar.memory.footprint_mb / 1024).toFixed(1)} / ${(sidecar.memory.cap_mb / 1024).toFixed(0)} GB`}
              />
              <StatRow label="Peak" value={`${(sidecar.memory.peak_mb / 1024).toFixed(1)} GB`} />
              <StatRow label="Memory recoveries" value={String(sidecar.memory.kills)} />
              {!sidecar.memory.measurement_complete ? (
                <p className="py-2 text-[11px] text-warning">
                  Memory measurement is unavailable; local processing cannot start safely.
                </p>
              ) : null}
              <StatRow
                label="Local queue"
                value={sidecar.quality_queued === 0 ? "Empty" : `${sidecar.quality_queued} waiting`}
              />
              {sidecar.parse.cloud_usage ? (
                <StatRow
                  label="MinerU usage today"
                  value={`${sidecar.parse.cloud_usage.pages} pages · ${sidecar.parse.cloud_usage.files} files`}
                />
              ) : null}
              <StatRow label="Parser" value={`v${sidecar.parser_version}`} />
            </>
          ) : null}
        </div>
      </Section>
    </>
  );
}
