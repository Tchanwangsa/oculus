import { indexedDraftStorage } from "./indexedStorage";
import { secureId } from "@/lib/canvas/ids";
import type { FetchLike } from "./api";

/** Everything the share client takes from the page, injected so tests drive it
 * with a fake server, a manual clock and no storage. */

export interface WebSocketLike {
  send(data: string): void;
  close(code?: number, reason?: string): void;
  onopen: ((event: Event) => void) | null;
  onmessage: ((event: MessageEvent) => void) | null;
  onclose: ((event: CloseEvent) => void) | null;
  onerror: ((event: Event) => void) | null;
}

export interface ShareClock {
  now(): number;
  setTimeout(callback: () => void, ms: number): unknown;
  clearTimeout(handle: unknown): void;
}

export type StorageLike = Pick<Storage, "getItem" | "setItem" | "removeItem">;
export interface DraftStorage {
  getItem(key: string): string | null | Promise<string | null>;
  setItem(key: string, value: string): void | Promise<void>;
  removeItem(key: string): void | Promise<void>;
}

/** Page lifecycle moments that matter to a connection: `visible` is a
 * `visibilitychange` to visible; `pagehide` flushes the draft. */
export type ShareLifecycleEvent = "online" | "visible" | "pageshow" | "pagehide";
export type ShareLifecycle = (listener: (event: ShareLifecycleEvent) => void) => () => void;

export interface ShareClientEnv {
  /** `http://<host>:<port>` of the page; the client talks to nothing else. */
  origin: string;
  fetch: FetchLike;
  openSocket(url: string): WebSocketLike;
  clock: ShareClock;
  random(): number;
  newId(): string;
  /** null when the browser refuses storage; every access is guarded anyway. */
  storage: DraftStorage | null;
  lifecycle: ShareLifecycle;
}

type Listen = (type: string, listener: () => void) => void;

/** The parts of `window` the client uses, so the end-to-end check
 * (`app/tests/e2e/`) runs this same factory over bun's globals. */
export interface SharePage {
  location: { origin: string };
  fetch: FetchLike;
  WebSocket: new (url: string) => WebSocketLike;
  setTimeout(callback: () => void, ms: number): unknown;
  clearTimeout(handle: number | undefined): void;
  readonly localStorage: StorageLike;
  readonly indexedDB?: IDBFactory;
  addEventListener: Listen;
  removeEventListener: Listen;
  document: { readonly visibilityState: string; addEventListener: Listen; removeEventListener: Listen };
}

function localStorageOrNull(page: SharePage): StorageLike | null {
  try { return page.localStorage; } catch { return null; }
}

function browserDraftStorage(page: SharePage): DraftStorage | null {
  try { return page.indexedDB ? indexedDraftStorage(page.indexedDB, localStorageOrNull(page)) : null; }
  catch { return null; }
}

/** The real page: same origin only. */
export function browserShareEnv(page: SharePage = window): ShareClientEnv {
  const { document } = page;
  return {
    origin: page.location.origin,
    fetch: (input, init) => page.fetch(input, init),
    openSocket: (url) => new page.WebSocket(url),
    clock: {
      now: () => Date.now(),
      setTimeout: (callback, ms) => page.setTimeout(callback, ms),
      clearTimeout: (handle) => page.clearTimeout(handle as number),
    },
    random: Math.random,
    newId: secureId,
    storage: browserDraftStorage(page),
    lifecycle(listener) {
      const online = () => listener("online");
      const pageshow = () => listener("pageshow");
      const pagehide = () => listener("pagehide");
      const visibility = () => { if (document.visibilityState === "visible") listener("visible"); };
      page.addEventListener("online", online);
      page.addEventListener("pageshow", pageshow);
      page.addEventListener("pagehide", pagehide);
      document.addEventListener("visibilitychange", visibility);
      return () => {
        page.removeEventListener("online", online);
        page.removeEventListener("pageshow", pageshow);
        page.removeEventListener("pagehide", pagehide);
        document.removeEventListener("visibilitychange", visibility);
      };
    },
  };
}

export const BACKOFF_MIN_MS = 500;
export const BACKOFF_MAX_MS = 8000;

/** Exponential from 0.5 s, up to a quarter longer at random so devices that lost
 * the Mac together don't return together, and never over 8 s. */
export function backoffDelay(attempt: number, random: () => number): number {
  const base = Math.min(BACKOFF_MAX_MS, BACKOFF_MIN_MS * 2 ** Math.max(0, attempt));
  return Math.min(BACKOFF_MAX_MS, Math.round(base * (1 + random() * 0.25)));
}
