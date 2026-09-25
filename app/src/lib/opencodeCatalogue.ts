/**
 * Which opencode models a picker may offer. opencode lists models that will
 * not answer; every filter here is free to check — capability
 * (`unusableReason`), the Zen rule (`isZen`) and the student's hides — because
 * a settings page may never make a billed call (see CLAUDE.md, "Nothing in
 * Settings may spend money"). A stale key is left to fail once in the timeline.
 *
 * Stored as one JSON value in `settings`; a module-level cache like
 * `useBridgeHealth`'s, since every picker reads it and only Settings writes it.
 */
import { useEffect, useState } from "react";

import { getSetting, setSetting } from "@/lib/db";

/** An entry exists only because the student unticked the model. */
export interface ModelEntry {
  hidden?: boolean;
}

interface ProviderEntry {
  /** Keyed by the full `providerID/id`; hidden models only. */
  models: Record<string, ModelEntry>;
}

export interface OpencodeCatalogue {
  /** The only way to remove a provider declared in the student's own
   *  opencode.json, which the app never edits. */
  hiddenProviders: string[];
  providers: Record<string, ProviderEntry>;
}

const CATALOGUE_KEY = "opencode_catalogue";

function empty(): OpencodeCatalogue {
  return { hiddenProviders: [], providers: {} };
}

/** Tolerant field by field, like `getJobModels`: only `hidden: true` survives
 *  per model; a malformed value costs the ticks, not the picker. */
function parse(raw: string | null): OpencodeCatalogue {
  if (!raw) return empty();
  try {
    const parsed = JSON.parse(raw);
    const out = empty();
    if (Array.isArray(parsed?.hiddenProviders)) {
      out.hiddenProviders = parsed.hiddenProviders.filter(
        (p: unknown): p is string => typeof p === "string" && p.length > 0,
      );
    }
    const providers = parsed?.providers;
    if (providers && typeof providers === "object") {
      for (const [providerId, entry] of Object.entries(providers)) {
        const e = entry as Partial<ProviderEntry> | null;
        const models: Record<string, ModelEntry> = {};
        if (e?.models && typeof e.models === "object") {
          for (const [modelId, m] of Object.entries(e.models)) {
            const row = m as Partial<ModelEntry> | null;
            if (row?.hidden === true) models[modelId] = { hidden: true };
          }
        }
        out.providers[providerId] = { models };
      }
    }
    return out;
  } catch {
    return empty();
  }
}

let cached: OpencodeCatalogue | null = null;
let inFlight: Promise<OpencodeCatalogue> | null = null;
let version = 0;
const listeners = new Set<(c: OpencodeCatalogue, v: number) => void>();

/** Read once and held; concurrent callers share one in-flight query. */
export function loadCatalogue(): Promise<OpencodeCatalogue> {
  if (cached) return Promise.resolve(cached);
  if (inFlight) return inFlight;
  const p = getSetting(CATALOGUE_KEY)
    // A failed read is not cached, so the next caller retries.
    .then((raw) => {
      const next = parse(raw);
      cached = next;
      inFlight = null;
      for (const l of listeners) l(next, version);
      return next;
    })
    .catch(() => {
      inFlight = null;
      return empty();
    });
  inFlight = p;
  return p;
}

/** Write, then update the cache from the written value (no re-read, so no
 *  frame of the old list). */
async function saveCatalogue(next: OpencodeCatalogue): Promise<void> {
  await setSetting(CATALOGUE_KEY, JSON.stringify(next));
  cached = next;
  version += 1;
  for (const l of listeners) l(next, version);
}

export function useCatalogue(): {
  /** Null until the first read lands. */
  catalogue: OpencodeCatalogue | null;
  save: (next: OpencodeCatalogue) => Promise<void>;
  version: number;
} {
  const [snap, setSnap] = useState<{ c: OpencodeCatalogue | null; v: number }>(() => ({
    c: cached,
    v: version,
  }));

  useEffect(() => {
    const on = (c: OpencodeCatalogue, v: number) => setSnap({ c, v });
    listeners.add(on);
    let live = true;
    void loadCatalogue().then((c) => {
      if (live) setSnap({ c, v: version });
    });
    return () => {
      live = false;
      listeners.delete(on);
    };
  }, []);

  return { catalogue: snap.c, save: saveCatalogue, version: snap.v };
}

/** **Absent means capable**; only an explicit `false` refuses. */
export interface ModelCapabilities {
  toolCall?: boolean;
  textInput?: boolean;
  textOutput?: boolean;
}

/** Why the agent cannot run this model, or null. A model without tool calls
 *  cannot read the course files and would answer from nothing, silently. */
export function unusableReason(m: ModelCapabilities): string | null {
  if (m.toolCall === false) {
    return "cannot call tools, so it cannot read your course files";
  }
  if (m.textInput === false) return "does not take text in";
  if (m.textOutput === false) return "does not answer in text";
  return null;
}

const ZEN_PROVIDER = "opencode";

/** opencode's free Zen models refuse every request not from the opencode TUI
 *  (HTTP 400), so none can run here. Settings still lists them, with why. */
export function isZen(modelId: string): boolean {
  return isZenProvider(providerOf(modelId));
}

export function isZenProvider(providerId: string): boolean {
  return providerId === ZEN_PROVIDER;
}

/** Everything before the **first** `/`, like Rust's `split_model`: model ids
 *  contain slashes of their own. No slash answers `""`. */
export function providerOf(modelId: string): string {
  const i = modelId.indexOf("/");
  return i > 0 ? modelId.slice(0, i) : "";
}

/** The id half of the gate; an absent entry means offered. */
function isOffered(modelId: string, c: OpencodeCatalogue): boolean {
  if (isZen(modelId)) return false;
  if (c.hiddenProviders.includes(providerOf(modelId))) return false;
  return c.providers[providerOf(modelId)]?.models[modelId]?.hidden !== true;
}

/** Both halves of the gate. Generic rather than typed to `HarnessModel` so
 *  this module never imports the harness — the dependency runs the other way. */
export function filterOffered<T extends { id: string } & ModelCapabilities>(
  models: T[],
  c: OpencodeCatalogue,
): T[] {
  return models.filter((m) => unusableReason(m) === null && isOffered(m.id, c));
}

/** Drop a provider's hides on disconnect, so reconnecting starts full. */
export function forgetProvider(c: OpencodeCatalogue, providerId: string): OpencodeCatalogue {
  const providers = { ...c.providers };
  delete providers[providerId];
  return {
    hiddenProviders: c.hiddenProviders.filter((p) => p !== providerId),
    providers,
  };
}
