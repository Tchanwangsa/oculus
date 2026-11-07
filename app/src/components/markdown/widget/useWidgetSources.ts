import { useEffect, useState } from "react";
import { knownKitBase, widgetKitBase } from "./kit";
import { transpile, transpiled, type Transpiled } from "./transpile";

/** `source` transpiled, or null while it is. */
export function useTranspiled(source: string): Transpiled | null {
  const [result, setResult] = useState<{ source: string; out: Transpiled } | null>(() => {
    const hit = transpiled(source);
    return hit ? { source, out: hit } : null;
  });
  useEffect(() => {
    let live = true;
    void transpile(source).then((out) => live && setResult({ source, out }));
    return () => {
      live = false;
    };
  }, [source]);
  return result?.source === source ? result.out : null;
}

export type KitBase = { base: string; error?: undefined } | { base?: undefined; error: string };

/** The kit's URL, or why it is unknown; null while asking. */
export function useKitBase(): KitBase | null {
  const [kit, setKit] = useState<KitBase | null>(() => {
    const base = knownKitBase();
    return base ? { base } : null;
  });
  useEffect(() => {
    if (kit) return;
    let live = true;
    widgetKitBase().then(
      (base) => live && setKit({ base }),
      (e) => live && setKit({ error: e instanceof Error ? e.message : String(e) }),
    );
    return () => {
      live = false;
    };
  }, [kit]);
  return kit;
}
