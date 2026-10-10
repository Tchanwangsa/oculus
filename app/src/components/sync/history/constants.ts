import type { SyncFileAction } from "@/lib/db";

export const ACTION_LABEL: Record<SyncFileAction, string> = {
  new: "Downloaded",
  updated: "Updated",
  unchanged: "Skipped",
};

export const ACTION_VARIANT: Record<SyncFileAction, "success" | "default" | "secondary"> = {
  new: "success",
  updated: "default",
  unchanged: "secondary",
};

/** Column template shared by the header and every row. */
// The run name is fixed-length, so the counts column takes the slack.
export const COLS =
  "grid grid-cols-[14px_200px_90px_80px_minmax(0,1fr)_100px] items-center gap-3 px-5";

export const INLINE_FILE_LIMIT = 8;
