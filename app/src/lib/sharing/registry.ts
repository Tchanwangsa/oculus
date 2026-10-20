/** One share controller per canvas session, fed every `companion-state`. A
 * share that disappears is checked even for a canvas no dialog has opened, so
 * a file whose share was stopped in Settings or ended natively goes back to
 * ordinary saves. */
import type { DbFile } from "@/lib/db";
import type { CanvasSession } from "@/lib/canvas/sessions";
import { canvasSessions } from "@/lib/canvas/sessionRuntime";
import { tauriSharingApi } from "./api";
import { shareController, shareFailure, type ShareController, type ShareFailure } from "./controller";
import { sharingStore, startSharingEvents } from "./store";

const controllers = new Map<number, { session: CanvasSession<DbFile>; controller: ShareController }>();

export function shareControllerFor(session: CanvasSession<DbFile>): ShareController {
  startSharingEvents();
  const entry = controllers.get(session.file.id);
  if (entry?.session === session) return entry.controller;
  const controller = shareController(session, tauriSharingApi);
  controllers.set(session.file.id, { session, controller });
  const state = sharingStore.get();
  if (state) controller.observe(state);
  return controller;
}

sharingStore.observe((state, previous) => {
  for (const share of previous?.shares ?? []) {
    if (state.shares.some((s) => s.fileId === share.fileId)) continue;
    const session = canvasSessions.get(share.fileId);
    if (session) shareControllerFor(session);
  }
  for (const { controller } of controllers.values()) controller.observe(state);
});

/** Stops one file's share, handing its session back to ordinary saves. */
export async function stopShare(fileId: number): Promise<ShareFailure | null> {
  const session = canvasSessions.get(fileId);
  if (session) return shareControllerFor(session).stop();
  try { await tauriSharingApi.stop(fileId); return null; }
  catch (error) { return shareFailure(error); }
}

/** Stops every share; each shared session sees its share gone and detaches. */
export async function stopAllShares(): Promise<ShareFailure | null> {
  try { await tauriSharingApi.stopAll(); return null; }
  catch (error) { return shareFailure(error); }
}

/** A shared canvas's title, while its session is open in this window. */
export const sharedFile = (fileId: number): DbFile | undefined => canvasSessions.get(fileId)?.file;
