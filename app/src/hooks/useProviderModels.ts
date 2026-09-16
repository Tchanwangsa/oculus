import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import type { PickerProvider } from "@/components/harness/ModelPicker";
import { providerHealth, useBridgeHealth } from "@/hooks/useBridgeHealth";
import { PROVIDERS, providerInfo, type HarnessModel, type Provider } from "@/lib/harness";

/**
 * The model picker's providers, with each one's catalogue and whether its CLI
 * is actually on this machine.
 *
 * Every picker in the app — the chat composer, the lecture dock's composer and
 * the per-job rows in Settings → AI — used to assemble this itself, in three
 * byte-for-byte copies of a fetch-gate plus a `claude ? static : fetched` map.
 * Three copies is three places to forget when an agent is added, and the map
 * was the kind of ternary that answers "not Claude" with the wrong list rather
 * than a type error. So it is one hook, and it names no provider at all: which
 * catalogues are compiled in and which are fetched is a property of the
 * `PROVIDERS` entry (`app/src/lib/harness.ts`), so a fourth agent is an entry
 * there and nothing here.
 *
 * `needed` is which providers are worth asking a CLI about, and it is a
 * parameter because the three call sites genuinely differ: a composer wants
 * the one provider its picker is showing, while the settings page wants every
 * provider some job row is set to — a page opened on a Codex job should not
 * spawn the other CLIs to fill in lists nobody is looking at. Everything else
 * about them was identical.
 *
 * A provider nobody has asked about reports `loading`, which is what it is:
 * its list is not here, and switching the picker to it is what asks. Each is
 * asked once per mount, failures included — a missing CLI answers with an
 * empty list, and re-asking it on every render of a menu would make an absent
 * agent slow as well as absent.
 *
 * **Health rides alongside the catalogue rather than replacing it.** A
 * provider whose binary was not found keeps the models it would have — Claude's
 * compiled-in list is still the list `claude --model` takes, and `modelsFor`
 * is used to resolve a selection, not to draw one — and it is the picker that
 * refuses to offer them. That keeps the gate in one place for all three, and
 * it is the same gate: before health lands, `unknown` means every provider
 * draws exactly as it drew before this existed. The one thing health does here
 * is spare a known-missing provider the fetch that could only fail.
 */
export function useProviderModels(needed: Provider | Provider[]): {
  providers: PickerProvider[];
  /** One provider's models, for a caller that has to resolve a selection for a
   *  provider that is not the one on screen — Settings switches a job's agent
   *  and has to pick that agent's default model in the same edit. */
  modelsFor: (p: Provider) => HarnessModel[];
} {
  const [fetched, setFetched] = useState<Partial<Record<Provider, HarnessModel[]>>>({});
  const asked = useRef(new Set<Provider>());
  const { health } = useBridgeHealth();

  // A stable key rather than the array itself: every call site builds its
  // `needed` inline, so a fresh array each render would re-run the effect
  // forever.
  const key = useMemo(
    () => (Array.isArray(needed) ? [...new Set(needed)].sort().join(" ") : needed),
    [needed],
  );

  useEffect(() => {
    for (const id of key.split(" ").filter(Boolean) as Provider[]) {
      const info = providerInfo(id);
      if (!info?.fetchModels || asked.current.has(id)) continue;
      // Known missing: the fetch would spawn a CLI that is not there and come
      // back empty, which the picker would have to tell apart from a real
      // empty answer. `unknown` still asks — health is not waited on, so a
      // composer's list arrives as fast as it ever did.
      if (providerHealth(health, id) === "missing") continue;
      asked.current.add(id);
      info
        .fetchModels()
        .then((models) => setFetched((f) => ({ ...f, [id]: models })))
        .catch(() => setFetched((f) => ({ ...f, [id]: [] })));
    }
  }, [key, health]);

  const providers = useMemo<PickerProvider[]>(
    () =>
      PROVIDERS.map((p) => {
        const state = providerHealth(health, p.id);
        return {
          id: p.id,
          label: p.label,
          models: p.staticModels ?? fetched[p.id] ?? [],
          // A missing provider is never "loading": nothing was asked, and
          // nothing is coming.
          loading: !p.staticModels && state !== "missing" && fetched[p.id] === undefined,
          health: state,
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
