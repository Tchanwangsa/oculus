import type { SourceNum } from "@/lib/db";

export interface DlProgress {
  mediaId: string;
  /** Which stream this bar belongs to; see `dlKey` in the download store. */
  source: SourceNum;
  percent: number;
  phase: string;
}

/** Window event: a player changed something the mounted lecture list shows. */
export const LECTURES_CHANGED_EVENT = "oculus:lectures-changed";
