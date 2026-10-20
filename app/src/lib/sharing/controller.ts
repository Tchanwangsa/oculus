/** The share dialog's flow for one canvas session, over injected commands, so
 * it runs on fakes in tests (docs/device-sharing.md, "Desktop UI"). Starting
 * hands the file to the canvas authority first, because the native side shares
 * only a live authority; stopping does the reverse. Every action resolves
 * (never rejects) with the failure it also shows. */
import { errorMessage, type CanvasAuthorityPhase } from "@/lib/canvas/authority";
import type { CompanionState, ShareEndpoints, ShareInfo, SharingApi } from "./types";

export interface ShareSessionPort {
  readonly file: { id: number };
  readonly authority: CanvasAuthorityPhase;
  attachAuthority(): Promise<void>;
  detachAuthority(): Promise<void>;
}

/** `unknown` until the first refresh says whether the file is shared. */
export type ShareStatus = "unknown" | "off" | "starting" | "sharing" | "stopping" | "ended";

export interface ShareFailure {
  /** The native `{code}` (`not-live`, `server`, `random`) or an authority code. */
  code?: string;
  message: string;
}

export interface ShareView {
  status: ShareStatus;
  /** The live share with a link per endpoint; null unless sharing. */
  share: ShareInfo | null;
  /** This Mac's addresses now, shared or not. */
  addresses: ShareEndpoints | null;
  /** The endpoint the user picked, kept while it is listed. */
  preferred: string | null;
  error: ShareFailure | null;
  /** Reading the shares or addresses failed; the share itself is untouched. */
  refreshError: string | null;
}

export interface ShareController {
  view(): ShareView;
  subscribe(listener: () => void): () => void;
  start(): Promise<ShareFailure | null>;
  stop(): Promise<ShareFailure | null>;
  stopAll(): Promise<ShareFailure | null>;
  /** Rereads the shares and addresses; never starts or stops anything. */
  refresh(): Promise<void>;
  /** Takes a `companion-state` event: addresses, client counts, a share the native side ended. */
  observe(state: CompanionState): void;
  select(endpointId: string): void;
  dismissError(): void;
}

export function shareFailure(error: unknown): ShareFailure {
  const code = typeof error === "object" && error !== null ? (error as { code?: unknown }).code : undefined;
  return { ...(typeof code === "string" ? { code } : {}), message: errorMessage(error) };
}

const holding = (phase: CanvasAuthorityPhase) => phase === "live" || phase === "paused";

export function shareController(session: ShareSessionPort, api: SharingApi): ShareController {
  const fileId = session.file.id;
  let view: ShareView = { status: "unknown", share: null, addresses: null, preferred: null, error: null, refreshError: null };
  const listeners = new Set<() => void>();
  let starting: Promise<ShareFailure | null> | undefined;
  let stopping: Promise<ShareFailure | null> | undefined;
  let refreshing: Promise<void> | undefined;
  let verifying: Promise<void> | undefined;
  /** The address generation the current links were read at. */
  let generation: number | undefined;

  const publish = (patch: Partial<ShareView>) => {
    view = { ...view, ...patch };
    for (const listener of listeners) listener();
  };
  const busy = () => !!starting || !!stopping;
  const settled = (status: ShareStatus): ShareStatus => (status === "unknown" ? "off" : status);
  const join = async (share: ShareInfo) => {
    try { if (!holding(session.authority)) await session.attachAuthority(); }
    catch (error) { publish({ refreshError: errorMessage(error) }); return; }
    if (busy()) return;
    generation = share.generation;
    publish({ status: "sharing", share });
  };

  /** The native side has no share for this file: hand the file back to ordinary saves. */
  const ended = async () => {
    publish({ status: view.status === "sharing" ? "ended" : "off", share: null });
    try { await session.detachAuthority(); }
    catch (error) { publish({ error: shareFailure(error) }); }
  };
  /** An event without our share may be older than our start, so ask before ending. */
  const verify = () => {
    verifying ??= (async () => {
      let shares: ShareInfo[];
      try { shares = await api.shares(); } catch { return; }
      if (busy()) return;
      const share = shares.find((s) => s.fileId === fileId && s.phase !== "ended");
      if (share) await join(share);
      else if (view.status === "sharing" || holding(session.authority)) await ended();
    })().finally(() => { verifying = undefined; });
    return verifying;
  };
  const halt = (call: () => Promise<void>) => {
    stopping ??= (async () => {
      const previous = view.status;
      publish({ status: "stopping", error: null });
      try { await call(); }
      catch (error) {
        const failure = shareFailure(error);
        publish({ status: previous, error: failure });
        return failure;
      }
      publish({ share: null });
      try { await session.detachAuthority(); }
      catch (error) {
        const failure = shareFailure(error);
        publish({ status: "off", error: failure });
        return failure;
      }
      publish({ status: "off" });
      return null;
    })().finally(() => { stopping = undefined; });
    return stopping;
  };

  const controller: ShareController = {
    view: () => view,
    subscribe(listener) { listeners.add(listener); return () => { listeners.delete(listener); }; },
    start() {
      if (stopping) return stopping.then(() => null);
      if (view.status === "sharing") return Promise.resolve(null);
      starting ??= (async () => {
        publish({ status: "starting", error: null });
        const attached = !holding(session.authority);
        try { await session.attachAuthority(); }
        catch (error) {
          const failure = shareFailure(error);
          publish({ status: "off", error: failure });
          return failure;
        }
        try {
          const share = await api.start(fileId);
          generation = share.generation;
          publish({ status: "sharing", share });
          return null;
        } catch (error) {
          const failure = shareFailure(error);
          if (attached) {
            try { await session.detachAuthority(); }
            catch (undo) { failure.message += ` Sharing could not be switched off either: ${errorMessage(undo)}`; }
          }
          publish({ status: "off", share: null, error: failure });
          return failure;
        }
      })().finally(() => { starting = undefined; });
      return starting;
    },
    stop() {
      if (starting) return starting.then(() => null);
      return halt(() => api.stop(fileId));
    },
    stopAll() {
      if (starting) return starting.then(() => null);
      return halt(() => api.stopAll());
    },
    refresh() {
      refreshing ??= (async () => {
        let shares: ShareInfo[], addresses: ShareEndpoints;
        try { [shares, addresses] = await Promise.all([api.shares(), api.endpoints()]); }
        catch (error) { publish({ status: busy() ? view.status : settled(view.status), refreshError: errorMessage(error) }); return; }
        publish({ addresses, refreshError: null });
        if (busy()) return;
        const share = shares.find((s) => s.fileId === fileId && s.phase !== "ended") ?? null;
        if (share) await join(share);
        else if (view.status === "sharing" || holding(session.authority)) await ended();
        else publish({ status: settled(view.status) });
      })().finally(() => { refreshing = undefined; });
      return refreshing;
    },
    observe(state) {
      publish({ addresses: { port: state.port, generation: state.generation, endpoints: state.endpoints } });
      if (busy()) return;
      const summary = state.shares.find((s) => s.fileId === fileId);
      if (!summary || summary.phase === "ended") {
        if (view.status === "sharing" || holding(session.authority)) void verify();
        return;
      }
      if (view.status !== "sharing" || !view.share || view.share.shareId !== summary.shareId) { void controller.refresh(); return; }
      const moved = summary.clients !== view.share.clients;
      if (moved || summary.phase !== view.share.phase) {
        publish({ share: { ...view.share, clients: summary.clients, phase: summary.phase } });
      }
      // The links, and each address's own device count, live only in `companion_shares`.
      if (state.generation !== generation || moved) {
        generation = state.generation;
        void controller.refresh();
      }
    },
    select(endpointId) { publish({ preferred: endpointId }); },
    dismissError() { publish({ error: null }); },
  };
  return controller;
}
