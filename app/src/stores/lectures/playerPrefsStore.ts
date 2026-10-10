import { create } from "zustand";
import { load, save } from "./playerPrefs/persist";
import type { PlayerPrefs } from "./playerPrefs/model";

export type { Dock, DockTab, Layout, PlayerPrefs } from "./playerPrefs/model";
export {
  PIP_MAX_W,
  PIP_MIN_PX_H,
  PIP_MIN_PX_W,
  PIP_MIN_W,
  SPEED_MAX,
  SPEED_MIN,
  SPEED_PRESETS,
  SPEED_STEP,
  clampOffset,
  clampPipWidth,
  clampSpeed,
  clampSplit,
  clampVolume,
  isVertical,
  reorderDockTabs,
} from "./playerPrefs/model";

interface PlayerPrefsState extends PlayerPrefs {
  set: (patch: Partial<PlayerPrefs>) => void;
}

export const usePlayerPrefs = create<PlayerPrefsState>((set, get) => ({
  ...load(),
  set: (patch) => {
    set(patch);
    const { set: _, ...prefs } = get();
    save(prefs);
  },
}));
