import { useCallback, useEffect, useRef, useState } from "react";

import {
  cachedProviders,
  opencodeDisconnect,
  opencodeProviders,
  rememberProviders,
  type OpencodeProvider,
  type OpencodeProviderList,
} from "@/lib/harness/opencodeAuth";
import {
  forgetProvider,
  isZen,
  providerOf,
  unusableReason,
  useCatalogue,
  type OpencodeCatalogue,
} from "@/lib/harness/opencodeCatalogue";
import { harnessOpencodeModels, opencodeAsModels, type HarnessModel } from "@/lib/harness";

/**
 * The opencode page's data: the provider list, opencode's model catalogue and
 * the stored ticks, with the writes both tables make. The first read is what
 * starts `opencode serve`, so nothing loads until `enabled` (the CLI is known
 * not to be missing). Nothing here makes a billed call (docs/harness.md, "No
 * model is ever probed").
 *
 * Model rows come from `harnessOpencodeModels()` (the unfiltered catalogue),
 * never the already-filtered picker list, or a hidden model could never be
 * unhidden.
 */
export function useOpencode(enabled: boolean) {
  const [list, setList] = useState<OpencodeProviderList | null>(cachedProviders);
  const [loading, setLoading] = useState(() => !cachedProviders());
  const [error, setError] = useState<string | null>(null);
  const [removing, setRemoving] = useState<string | null>(null);
  const { catalogue, save } = useCatalogue();

  const loadList = useCallback(async (refresh: boolean) => {
    setLoading(true);
    setError(null);
    try {
      setList(rememberProviders(await opencodeProviders(refresh)));
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    if (enabled && !cachedProviders()) void loadList(false);
  }, [enabled, loadList]);

  /** opencode's whole catalogue (every provider, one call), held for the
   *  page's life; the refs dedupe loads without a stale closure. */
  const modelsRef = useRef<HarnessModel[] | null>(null);
  const inFlight = useRef<Promise<HarnessModel[]> | null>(null);
  const [models, setModels] = useState<HarnessModel[] | null>(null);

  const loadModels = useCallback(async (force: boolean): Promise<HarnessModel[]> => {
    if (!force && modelsRef.current) return modelsRef.current;
    if (!force && inFlight.current) return inFlight.current;
    const p = harnessOpencodeModels()
      .then((raw) => {
        const next = opencodeAsModels(raw);
        modelsRef.current = next;
        setModels(next);
        inFlight.current = null;
        return next;
      })
      .catch((e) => {
        inFlight.current = null;
        setError(String(e));
        return [] as HarnessModel[];
      });
    inFlight.current = p;
    return p;
  }, []);

  // Read on open: the provider rows' offered counts need it too.
  useEffect(() => {
    if (enabled) void loadModels(false);
  }, [enabled, loadModels]);

  const disconnect = useCallback(
    async (provider: OpencodeProvider) => {
      setRemoving(provider.id);
      setError(null);
      try {
        setList(rememberProviders(await opencodeDisconnect(provider.id)));
        // Reconnecting later starts from a clean catalogue entry, and the
        // held model list still has this provider's rows.
        if (catalogue) await save(forgetProvider(catalogue, provider.id));
        void loadModels(true);
      } catch (e) {
        setError(String(e));
      } finally {
        setRemoving(null);
      }
    },
    [catalogue, save, loadModels],
  );

  /** A `config` provider comes from the student's own `opencode.json`, which
   *  Oculus never edits, so hiding is the only way to remove it. */
  const setProviderHidden = useCallback(
    async (providerId: string, hidden: boolean) => {
      if (!catalogue) return;
      const rest = catalogue.hiddenProviders.filter((p) => p !== providerId);
      await save({ ...catalogue, hiddenProviders: hidden ? [...rest, providerId] : rest });
    },
    [catalogue, save],
  );

  const setModelsHidden = useCallback(
    async (ids: string[], hidden: boolean) => {
      if (!catalogue || ids.length === 0) return;
      await save(withHidden(catalogue, ids, hidden));
    },
    [catalogue, save],
  );

  /** After a connect: the held catalogue predates the new provider. */
  const connected = useCallback(
    (next: OpencodeProviderList) => {
      setList(rememberProviders(next));
      void loadModels(true);
    },
    [loadModels],
  );

  return {
    list,
    loading,
    error,
    removing,
    catalogue,
    models,
    loadList,
    disconnect,
    setProviderHidden,
    setModelsHidden,
    connected,
  };
}

/** Why this model is never offered, or null (see `lib/harness/opencodeCatalogue.ts`).
 *  Zen first: those models claim every capability and still refuse. */
export function blockedBecause(model: HarnessModel): string | null {
  if (isZen(model.id)) {
    return "opencode’s free tier only runs inside opencode itself, so Chat cannot use it.";
  }
  const reason = unusableReason(model);
  return reason === null ? null : `This model ${reason}.`;
}

/** Whether the student unticked this model. */
export function isHidden(c: OpencodeCatalogue | null, modelId: string): boolean {
  return c?.providers[providerOf(modelId)]?.models[modelId]?.hidden === true;
}

/** Set or clear `hidden` on models of any providers. An absent entry means
 *  offered, so showing deletes the entry rather than writing `false`. */
function withHidden(c: OpencodeCatalogue, modelIds: string[], hidden: boolean): OpencodeCatalogue {
  const providers = { ...c.providers };
  for (const id of modelIds) {
    const providerId = providerOf(id);
    const entry = providers[providerId] ?? { models: {} };
    const models = { ...entry.models };
    if (hidden) models[id] = { hidden: true };
    else delete models[id];
    providers[providerId] = { ...entry, models };
  }
  return { ...c, providers };
}
