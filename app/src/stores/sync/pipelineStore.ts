import { create } from "zustand";
import { newItem, settleError, inFlight, FRESH_MS } from "./pipeline/item";
import type { PipelineItem, StagePatch } from "./pipeline/item";
import { mergeSeed, seededItem, type SeedRow } from "./pipeline/seed";
import { hasFailed, isComplete } from "./pipeline/status";

/**
 * Per-file view of the ingest pipeline: download → parse → embed. The embed
 * stage applies only when a Voyage key is stored (`embedStage`), and only to
 * PDF-backed files (`embedsIn`); otherwise a parsed file is finished. A
 * spreadsheet's parse is its conversion to text.
 *
 * Fed by `useBackendEvents` (scrape-file-start, scrape-file, parse-status,
 * embed-status) and seeded from the DB on the Sync page. A finished parse is
 * `"quality"` on the wire and in `files.parse_status`; a finished embed is
 * `"done"`. A parse the user skipped is `"skipped"` in both, and the row's
 * parse stage; it is settled, never a failure and never paused.
 */

export type { StageState, ParsePhase, PipelineItem, StagePatch } from "./pipeline/item";
export { NO_UPLOAD, runningPatch } from "./pipeline/item";
export { fmtEta, uploadEta } from "./pipeline/eta";
export {
  embedsIn,
  fmtMb,
  fmtResume,
  fmtWait,
  hasFailed,
  isComplete,
  statusOf,
  type PipelinePhase,
  type StatusView,
} from "./pipeline/status";

interface PipelineState {
  items: Record<string, PipelineItem>;
  /** False until a Voyage key is stored, so no phantom embed backlog is drawn.
   *  Set from `indexStore`. */
  embedStage: boolean;
  setEmbedStage: (on: boolean) => void;
  /** Paths "Clear finished" removed; `seed` leaves them out while they are
   *  still complete, and a live `touch` brings them back. */
  cleared: Record<string, true>;

  /** Merge a stage update, creating the row if this is the first sighting. */
  touch: (relativePath: string, subjectId: number, patch: StagePatch) => void;
  /** A sync found the file unchanged on disk. Not a sign of movement: never
   *  creates a row, and only settles a download the row still shows as
   *  unfinished. */
  confirmDownload: (relativePath: string) => void;
  /** Merge the DB's view: new rows are added, idle rows advance to where the
   *  DB is further along, and rows for files no longer on record are dropped. */
  seed: (rows: SeedRow[]) => void;
  /** Mark rows stuck mid-download as failed (a run ended without their bytes). */
  failStalledDownloads: () => void;
  /** Drop completed rows. Failed ones stay, since they need attention. */
  clearFinished: () => void;
}

export const usePipelineStore = create<PipelineState>((set, get) => ({
  items: {},
  embedStage: false,
  cleared: {},

  setEmbedStage: (on) => {
    if (get().embedStage !== on) set({ embedStage: on });
  },

  touch: (relativePath, subjectId, patch) =>
    set((s) => {
      const prev = s.items[relativePath] ?? newItem(relativePath, subjectId);
      // A parse or embed event means the bytes are on disk, even when it is
      // the first event this session saw for the file, or a run marked its
      // download failed.
      const onDisk =
        (prev.download === "pending" || prev.download === "error") &&
        patch.download === undefined &&
        (patch.parse !== undefined || patch.embed !== undefined);
      let next: PipelineItem = {
        ...prev,
        ...(onDisk ? { download: "done" as const } : {}),
        ...patch,
        // Keep a subject id we already know over a missing one.
        subjectId: prev.subjectId || subjectId,
        // Any live event means the file is moving again.
        paused: false,
        updatedAt: Date.now(),
      };
      if (prev.download === "error" && next.download === "done") next = settleError(next);
      let cleared = s.cleared;
      if (cleared[relativePath]) {
        cleared = { ...cleared };
        delete cleared[relativePath];
      }
      return { items: { ...s.items, [relativePath]: next }, cleared };
    }),

  confirmDownload: (relativePath) =>
    set((s) => {
      const prev = s.items[relativePath];
      if (!prev || prev.download === "done") return {};
      const next = settleError({ ...prev, download: "done" });
      return { items: { ...s.items, [relativePath]: next } };
    }),

  seed: (rows) =>
    set((s) => {
      const now = Date.now();
      const items: Record<string, PipelineItem> = {};
      const cleared: Record<string, true> = {};
      for (const r of rows) {
        const db = seededItem(r);
        const live = s.items[r.relativePath];
        if (live) {
          items[r.relativePath] = mergeSeed(live, db, now);
        } else if (s.cleared[r.relativePath] && isComplete(db, s.embedStage)) {
          cleared[r.relativePath] = true;
        } else {
          items[r.relativePath] = db;
        }
      }
      // A file gone from the DB was deleted or renamed, unless its row is
      // still being written.
      for (const [k, it] of Object.entries(s.items)) {
        if (k in items) continue;
        if (inFlight(it) || now - it.updatedAt < FRESH_MS) items[k] = it;
      }
      return { items, cleared };
    }),

  failStalledDownloads: () =>
    set((s) => {
      const items = { ...s.items };
      for (const [k, it] of Object.entries(items)) {
        if (it.download === "active") {
          items[k] = {
            ...it,
            download: "error",
            error: "Download did not complete",
            updatedAt: Date.now(),
          };
        }
      }
      return { items };
    }),

  clearFinished: () =>
    set((s) => {
      const items: Record<string, PipelineItem> = {};
      const cleared = { ...s.cleared };
      for (const [k, it] of Object.entries(s.items)) {
        if (isComplete(it, s.embedStage) && !hasFailed(it, s.embedStage)) cleared[k] = true;
        else items[k] = it;
      }
      return { items, cleared };
    }),
}));
