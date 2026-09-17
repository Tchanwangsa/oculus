/**
 * Which opencode models a picker is allowed to offer.
 *
 * opencode reaches 218 providers, and its catalogue is a list of what the CLI
 * *knows about* rather than of what will answer: measured against opencode
 * 1.18.2, the free Zen models come back
 * `HTTP 400 "OpenCode's free tier can only be used in OpenCode"`, and a
 * provider declared in the student's own `opencode.json` with a stale key
 * comes back `HTTP 401` — both listed, neither usable.
 *
 * There used to be a **probe** for exactly that: one real call per model, and
 * a model that had not answered one was not offered. It worked and it is
 * deleted, because the evidence was bought — a session and a prompt per
 * model, billed by the provider, twice over (opencode titles each session it
 * creates), against a provider like OpenRouter's ~300 models. A settings page
 * may not spend a student's credits to populate a menu.
 *
 * **Most of what it bought was already free**, which is the part worth
 * keeping:
 *
 * - *Is the provider signed in?* `/config/providers` answers it. It lists the
 *   providers the config can actually **use** — three of 221 on the machine
 *   this was measured on — so a provider with no credential contributes no
 *   models to the list in the first place. Nothing had to be asked.
 * - *Does the model exist?* Same list, same answer. A model that is in it
 *   resolves; the probe's `Model not found` class cannot arise from a menu
 *   built out of the list itself.
 * - *Is it a Zen model?* A **static** fact about the id — see `isZen` below.
 *
 * What is left over is a credential that is present but stale: a 401 nothing
 * free can see. That one fails once, in the timeline, in the provider's own
 * words, which `friendly` in `app/src-tauri/src/harness/opencode.rs` already
 * writes into a sentence. One failed message is the honest price; ~600 billed
 * requests was not.
 *
 * So three filters are left, and none of them costs anything: **capability**
 * (`unusableReason`), the **Zen** rule, and **hiding** — the student unticking
 * rows in Settings, or a whole provider.
 *
 * The whole thing is one JSON value in `settings`, read and written the way
 * `getJobModels`/`setJobModels` do it (`app/src/lib/db.ts`) and just as
 * tolerant of a value an older build wrote: a malformed catalogue costs a
 * student their ticks, not their picker.
 *
 * Module state is modelled on `app/src/hooks/useBridgeHealth.ts` for the same
 * reason that one is: this is one value read by every model picker in the app
 * and edited in exactly one place, so it is a module-level cache, one
 * in-flight promise and a set of listeners rather than a store.
 */
import { useEffect, useState } from "react";

import { getSetting, setSetting } from "@/lib/db";

/**
 * What the student decided about one model, and nothing else.
 *
 * This used to carry a probe's verdict — `ok`, the provider's refusal, when it
 * was asked — because a model reached the picker only once a real turn had
 * been sent to it and had answered. That sweep was **billed**: one session and
 * one prompt per model, against a provider like OpenRouter's three hundred, at
 * whatever that provider charges. It is gone, and so is everything it wrote.
 * An entry exists now only because a model was unticked.
 */
export interface ModelEntry {
  /** The student unticked it in Settings. */
  hidden?: boolean;
}

export interface ProviderEntry {
  /** Keyed by the full `providerID/id` the CLI takes back. Only the hidden
   *  ones: a model opencode lists and nobody has hidden has no row here. */
  models: Record<string, ModelEntry>;
}

export interface OpencodeCatalogue {
  /** Providers the student removed from Oculus. A `config` provider declared
   *  in their own opencode.json cannot be disconnected, so this is the only
   *  way to get it out of the picker — and the app never edits their config. */
  hiddenProviders: string[];
  providers: Record<string, ProviderEntry>;
}

const CATALOGUE_KEY = "opencode_catalogue";

function empty(): OpencodeCatalogue {
  return { hiddenProviders: [], providers: {} };
}

/**
 * Tolerant on read, field by field, like `getJobModels`: anything that is not
 * the shape this build writes is dropped rather than carried forward.
 *
 * It reads catalogues written by the probe build too, and that is the point of
 * the one filter below: those rows carry `ok`/`at`/`error` from a sweep, and
 * the only field of them that was ever the student's own choice is `hidden`.
 * Everything else is discarded on the first read, so a stored verdict cannot
 * keep a model out of the picker now that nothing re-checks it.
 */
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

/** The stored catalogue, read once and then held. Every opencode fetch waits
 *  on this, so ten pickers mounting at once make one query rather than ten. */
export function loadCatalogue(): Promise<OpencodeCatalogue> {
  if (cached) return Promise.resolve(cached);
  if (inFlight) return inFlight;
  const p = getSetting(CATALOGUE_KEY)
    // A failed read is an empty catalogue for this attempt only: it is not
    // cached, so the next picker asks again rather than inheriting a blank
    // menu from one query that lost a race with the database opening.
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

/**
 * Replace the stored catalogue, then tell everyone holding it.
 *
 * The cache is replaced from the value that was written rather than re-read,
 * for the same reason there is only one writer: Settings owns this value, and
 * a round trip would let a picker draw the old list for a frame after a tick.
 */
export async function saveCatalogue(next: OpencodeCatalogue): Promise<void> {
  await setSetting(CATALOGUE_KEY, JSON.stringify(next));
  cached = next;
  version += 1;
  for (const l of listeners) l(next, version);
}

/** Bumped on every save and never reset. A consumer that has to *redo* work
 *  when this changes — `useProviderModels` re-asks the CLI — compares this
 *  instead of diffing two catalogues to find out whether the answer it is
 *  holding could still be right. */
export function catalogueVersion(): number {
  return version;
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

/** The provider half of an opencode model id — everything before the **first**
 *  `/`. The rest is free-form and routinely contains more slashes of its own
 *  (`tss-nvidia-spark/nvidia/Qwen3.6-35B-A3B-NVFP4`), which is why Rust's
 *  `split_model` (`app/src-tauri/src/harness/opencode.rs`) splits once too.
 *  Splitting on the last one, or on all of them, invents a provider nobody
 *  has. An id with no slash at all has no provider and answers `""`. */
/** What the catalogue claims a model can do. Every field is optional and
 *  **absent means capable**: Claude's and Codex's lists carry none of them,
 *  and a provider whose rows say nothing about tools must not have its whole
 *  catalogue gated out of the picker. Only an explicit `false` refuses. */
export interface ModelCapabilities {
  toolCall?: boolean;
  textInput?: boolean;
  textOutput?: boolean;
}

/**
 * Why Oculus's agent cannot run this model, or null when it can — the second
 * gate that costs nothing to ask.
 *
 * **Tool calling is the one that matters.** The agent answers out of the
 * student's own library: it reads `AGENTS.md`, greps the course folder, opens
 * the markdown a parse produced. A model that cannot call a tool cannot do any
 * of that, so it does not fail loudly — it answers from nothing, confidently,
 * which is worse than a red row. Measured on this machine's catalogue: **69 of
 * OpenRouter's 369** are in this state, and none of opencode's or the Spark
 * endpoint's.
 *
 * The text flags are assertions rather than filters — every model in the
 * catalogue today takes and answers in text — kept because the list grows.
 * OpenRouter's own catalogue already carries transcription, embedding, rerank
 * and image models; the day opencode surfaces one, it should not reach a chat
 * composer.
 */
export function unusableReason(m: ModelCapabilities): string | null {
  if (m.toolCall === false) {
    return "cannot call tools, so it cannot read your course files";
  }
  if (m.textInput === false) return "does not take text in";
  if (m.textOutput === false) return "does not answer in text";
  return null;
}

/** opencode's own free tier, whose provider id is exactly this. */
const ZEN_PROVIDER = "opencode";

/**
 * Whether a model is one of opencode's free **Zen** models, which this app can
 * never run — and the one gateway refusal that is knowable for free.
 *
 * It is not a guess about a flaky provider: the Zen gateway answers
 * `HTTP 400 … MissingSessionID … "OpenCode's free tier can only be used in
 * OpenCode"` to anything that is not the opencode TUI, so *every* model under
 * this provider refuses *every* request from the bridge, always. The probe
 * used to learn that one model at a time, at a cost, for a fact that is
 * structural and free to read off the id.
 *
 * These stay visible in Settings rather than being filtered out of the list:
 * a student who signed in to opencode's free tier and cannot find its models
 * is owed the reason, not an absence.
 */
export function isZen(modelId: string): boolean {
  return isZenProvider(providerOf(modelId));
}

/** [`isZen`] for a whole provider, since every model under it is one. */
export function isZenProvider(providerId: string): boolean {
  return providerId === ZEN_PROVIDER;
}

export function providerOf(modelId: string): string {
  const i = modelId.indexOf("/");
  return i > 0 ? modelId.slice(0, i) : "";
}

/**
 * The **catalogue** half of the rule: a model is offered when it is not a Zen
 * model, its provider is not hidden, and the student has not unticked it. The
 * capability half is `unusableReason`, which needs the model row rather than
 * its id — [`filterOffered`] is where the two meet, and the composer only ever
 * reaches the picker through that.
 *
 * **An absent entry means offered**, which is the reverse of what this said
 * while the probe existed. Back then a model had to have answered a real turn
 * before the composer would show it, so "no entry" meant "not checked yet" and
 * the safe reading was to withhold it. That reading was only affordable
 * because something was paying per model to remove the doubt.
 */
export function isOffered(modelId: string, c: OpencodeCatalogue): boolean {
  if (isZen(modelId)) return false;
  if (c.hiddenProviders.includes(providerOf(modelId))) return false;
  return c.providers[providerOf(modelId)]?.models[modelId]?.hidden !== true;
}

/** Both halves of the gate over a list, and the only place they are applied
 *  together. Generic on the shape rather than typed to `HarnessModel`, so the
 *  catalogue never has to import the harness — the dependency runs the other
 *  way. */
export function filterOffered<T extends { id: string } & ModelCapabilities>(
  models: T[],
  c: OpencodeCatalogue,
): T[] {
  return models.filter((m) => unusableReason(m) === null && isOffered(m.id, c));
}

/** Drop everything remembered about a provider — every hidden flag under it.
 *  Disconnecting is the student saying the credential is gone, so the ticks
 *  they made against it are no longer about anything; reconnecting later
 *  should start from a full menu rather than inherit one emptied earlier. */
export function forgetProvider(c: OpencodeCatalogue, providerId: string): OpencodeCatalogue {
  const providers = { ...c.providers };
  delete providers[providerId];
  return {
    hiddenProviders: c.hiddenProviders.filter((p) => p !== providerId),
    providers,
  };
}
