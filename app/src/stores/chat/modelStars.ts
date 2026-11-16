import { create } from "zustand";
import { persist } from "zustand/middleware";
import type { Provider } from "@/lib/harness";

/** A model the student starred in the model picker. The label is kept so a
 *  star still reads well while its agent's catalogue is loading. */
export interface ModelStar {
  provider: Provider;
  model: string;
  label: string;
}

const same = (a: ModelStar, provider: Provider, model: string) => a.provider === provider && a.model === model;

/** Starred models across every agent, in the order they were starred; the
 *  model picker's Starred tab lists them. Kept in this webview's storage. */
export const useModelStars = create<{
  stars: ModelStar[];
  toggle: (star: ModelStar) => void;
}>()(
  persist(
    (set) => ({
      stars: [],
      toggle: (star) =>
        set((s) => ({
          stars: s.stars.some((x) => same(x, star.provider, star.model))
            ? s.stars.filter((x) => !same(x, star.provider, star.model))
            : [...s.stars, star],
        })),
    }),
    { name: "oculus-model-stars" },
  ),
);

export const isStarred = (stars: ModelStar[], provider: Provider, model: string) =>
  stars.some((x) => same(x, provider, model));
