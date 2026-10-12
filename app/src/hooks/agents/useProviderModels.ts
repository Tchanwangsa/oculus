import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import { providerHealth, useBridgeHealth } from "@/hooks/agents/useBridgeHealth";
import { useWindowEvent } from "@/hooks/backend/useEvents";
import { PROVIDERS, providerAvailability, providerInfo, type HarnessModel, type Provider, type PickerProvider } from "@/lib/harness";
import { useCatalogue } from "@/lib/harness/opencodeCatalogue";

/**
 * Every picker's providers, each with its catalogue and CLI health. Names no
 * provider: everything comes off `PROVIDERS`. `needed` is which CLIs are worth
 * asking — each is asked once per mount (failures included), re-armed when the
 * opencode catalogue's version changes or its CLI is replaced (an update moves
 * a CLI's list). A known-missing CLI is not asked;
 * the picker, not this hook, refuses to offer its models.
 *
 * `start: false` asks a provider with `peekModels` through it, so a CLI only
 * its own Settings page may start (opencode) is read only if already running;
 * found down, it is asked again on its `runningEvent`.
 */
export function useProviderModels(
  needed: Provider | Provider[],
  { start = true }: { start?: boolean } = {},
): {
  providers: PickerProvider[];
  /** Models for a provider not on screen, to resolve a selection. */
  modelsFor: (p: Provider) => HarnessModel[];
} {
  const [fetched, setFetched] = useState<Partial<Record<Provider, HarnessModel[]>>>({});
  // Asked through `peekModels` and found not running.
  const [dormant, setDormant] = useState<ReadonlySet<Provider>>(new Set());
  const asked = useRef(new Set<Provider>());
  // The CLI each answer came from, as `path@version`.
  const askedBuild = useRef(new Map<Provider, string>());
  const { health } = useBridgeHealth();
  const { version } = useCatalogue();
  const askedVersion = useRef(version);
  // Bumped to re-run the asking effect after a dormant provider starts.
  const [woken, setWoken] = useState(0);

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
      if (!info?.fetchModels || !providerAvailability(id).supported) continue;
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
      const ask = !start && info.peekModels ? info.peekModels : info.fetchModels;
      ask()
        .then((models) => {
          setFetched((f) => ({ ...f, [id]: models ?? [] }));
          setDormant((d) => {
            if (d.has(id) === (models === null)) return d;
            const next = new Set(d);
            if (models === null) next.add(id);
            else next.delete(id);
            return next;
          });
        })
        .catch(() => setFetched((f) => ({ ...f, [id]: [] })));
    }
  }, [key, health, version, start, woken]);

  const runningEvents = useMemo(
    () => PROVIDERS.flatMap((p) => (p.runningEvent ? [p.runningEvent] : [])),
    [],
  );
  useWindowEvent(runningEvents, (e) => {
    const woke = PROVIDERS.filter((p) => p.runningEvent === e.type && dormant.has(p.id));
    if (woke.length === 0) return;
    for (const p of woke) asked.current.delete(p.id);
    setWoken((n) => n + 1);
  });

  const providers = useMemo<PickerProvider[]>(
    () =>
      PROVIDERS.filter((p) => providerAvailability(p.id).supported).map((p) => {
        const state = providerHealth(health, p.id);
        return {
          id: p.id,
          label: p.label,
          models: p.staticModels ?? fetched[p.id] ?? [],
          loading: !p.staticModels && state !== "missing" && fetched[p.id] === undefined,
          health: state,
          emptyNote: dormant.has(p.id) ? p.dormantNote : p.emptyNote,
          emptyNotePage: p.emptyNotePage,
        };
      }),
    [fetched, health, dormant],
  );

  const modelsFor = useCallback(
    (p: Provider) => providers.find((x) => x.id === p)?.models ?? [],
    [providers],
  );

  return { providers, modelsFor };
}
