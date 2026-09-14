import { create } from "zustand";

/**
 * Whether the ⌘K palette is up.
 *
 * A store rather than local state in `AppLayout` because the two things that
 * open it are far apart: the menu event the palette listens for itself, and the
 * sidebar's Search row. Nothing else needs to know it exists.
 */
interface PaletteState {
  open: boolean;
  setOpen: (open: boolean) => void;
  toggle: () => void;
}

export const usePaletteStore = create<PaletteState>((set) => ({
  open: false,
  setOpen: (open) => set({ open }),
  toggle: () => set((s) => ({ open: !s.open })),
}));
