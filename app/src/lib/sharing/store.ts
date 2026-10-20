/** The latest `companion-state`, app-wide: one listener, started on first use
 * and kept for the app's life, seeded from the commands until the first event
 * arrives (one is sent on every share or address change, none before). */
import { useSyncExternalStore } from "react";
import { listen } from "@tauri-apps/api/event";
import { COMPANION_STATE_EVENT, tauriSharingApi } from "./api";
import type { CompanionState, ShareSummary, SharingApi } from "./types";

type Observer = (state: CompanionState, previous: CompanionState | null) => void;

let current: CompanionState | null = null;
let received = false;
let started = false;
const listeners = new Set<() => void>();
const observers = new Set<Observer>();

function set(state: CompanionState) {
  const previous = current;
  current = state;
  for (const listener of listeners) listener();
  for (const observer of observers) observer(state, previous);
}

/** Sharing state as the commands report it, for before the first event. */
export async function readCompanionState(api: SharingApi): Promise<CompanionState> {
  const [shares, addresses] = await Promise.all([api.shares(), api.endpoints()]);
  return {
    running: addresses.port != null,
    port: addresses.port,
    ipv6: addresses.endpoints.some((e) => e.family === "ipv6" && e.url),
    generation: addresses.generation,
    endpoints: addresses.endpoints,
    shares: shares.map(({ fileId, shareId, epoch, clients, phase }): ShareSummary => ({ fileId, shareId, epoch, clients, phase })),
  };
}

export function startSharingEvents() {
  if (started) return;
  started = true;
  listen<CompanionState>(COMPANION_STATE_EVENT, ({ payload }) => { received = true; set(payload); })
    .then(() => readCompanionState(tauriSharingApi))
    .then((state) => { if (!received) set(state); })
    .catch(() => { if (!received) started = false; });
}

export const sharingStore = {
  get: () => current,
  subscribe(listener: () => void) { listeners.add(listener); return () => { listeners.delete(listener); }; },
  /** Called after every new state with the one before it. */
  observe(observer: Observer) { observers.add(observer); return () => { observers.delete(observer); }; },
};

/** The latest sharing state, and `fileId`'s share in it when given. */
export function useSharing(fileId?: number): { state: CompanionState | null; share: ShareSummary | undefined } {
  startSharingEvents();
  const state = useSyncExternalStore(sharingStore.subscribe, sharingStore.get);
  return { state, share: fileId == null ? undefined : state?.shares.find((s) => s.fileId === fileId) };
}
