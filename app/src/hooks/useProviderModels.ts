import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import { providerHealth, useBridgeHealth } from "@/hooks/useBridgeHealth";
import { PROVIDERS, providerInfo, type HarnessModel, type Provider, type PickerProvider } from "@/lib/harness";
import { useCatalogue } from "@/lib/opencodeCatalogue";

/**
 * Every picker's providers, each with its catalogue and CLI health. Names no
 * provider: everything comes off `PROVIDERS`. `needed` is which CLIs are worth
 * asking — each is asked once per mount (failures included), re-armed when the
 * opencode catalogue's version changes or its CLI is replaced (an update moves
 * a CLI's list). A known-missing CLI is not asked;
 * the picker, not this hook, refuses to offer its models.
 */
export function useProviderModels(needed: Provider | Provider[]): {
  providers: PickerProvider[];
  /** Models for a provider not on screen, to resolve a selection. */
  modelsFor: (p: Provider) => HarnessModel[];
} {
  const [fetched, setFetched] = useState<Partial<Record<Provider, HarnessModel[]>>>({});
  const asked = useRef(new Set<Provider>());
  // The CLI each answer came from, as `path@version`.
  const askedBuild = useRef(new Map<Provider, string>());
  const { health } = useBridgeHealth();
  const { version } = useCatalogue();
  const askedVersion = useRef(version);

  // A stable key: call sites build `needed` inline.
  const key = useMemo(
    () => (Array.isArray(needed) ? [...new Set(needed)].sort().join(" ") : needed),
    [needed],
  );

  useEffect(() => {
    // Re-armed here, not in its own effect, so effect order cannot matter.
    if (askedVersion.current !== version) {
      askedVersion.current = version;
      asked.current.clear();
    }
    for (const id of key.split(" ").filter(Boolean) as Provider[]) {
      const info = providerInfo(id);
      if (!info?.fetchModels) continue;
      const row = health?.find((h) => h.provider === id);
      const build = row?.path ? `${row.path}@${row.version ?? ""}` : "";
      // `unknown` health has no build yet; only a known build that changed re-asks.
      const before = askedBuild.current.get(id);
      if (build && before !== build) {
        if (before) asked.current.delete(id);
        askedBuild.current.set(id, build);
      }
      if (asked.current.has(id)) continue;
      // `unknown` still asks: health is not waited on.
      if (providerHealth(health, id) === "missing") continue;
      asked.current.add(id);
      askedBuild.current.set(id, build);
      info
        .fetchModels()
        .then((models) => setFetched((f) => ({ ...f, [id]: models })))
        .catch(() => setFetched((f) => ({ ...f, [id]: [] })));
    }
  }, [key, health, version]);

  const providers = useMemo<PickerProvider[]>(
    () =>
      PROVIDERS.map((p) => {
        const state = providerHealth(health, p.id);
        return {
          id: p.id,
          label: p.label,
          models: p.staticModels ?? fetched[p.id] ?? [],
          loading: !p.staticModels && state !== "missing" && fetched[p.id] === undefined,
          health: state,
          emptyNote: p.emptyNote,
          emptyNotePage: p.emptyNotePage,
        };
      }),
    [fetched, health],
  );

  const modelsFor = useCallback(
    (p: Provider) => providers.find((x) => x.id === p)?.models ?? [],
    [providers],
  );

  return { providers, modelsFor };
}
