import { isSheetFile } from "@/lib/files/fileTypes";
import { getFileByRelativePath } from "@/lib/db";
import { filePagePath, openFileSmart } from "@/lib/files/openFile";
import { openBeside } from "@/lib/shell/tabRouters";
import type { PipelineItem, StatusView } from "@/stores/sync/pipelineStore";

export interface RowActions {
  /** ▶: resume, retry, embed or parse a skipped file. */
  run?: { label: string; hint: string };
  skip: boolean;
}

export function actionsOf(
  item: PipelineItem,
  s: StatusView,
  embedStage: boolean,
  canRun: boolean,
  canSkip: boolean,
): RowActions {
  // A parsed-but-unembedded file gets ▶ too: the backlog isn't swept
  // automatically, so this embeds one file without committing the library.
  const embedNow = embedStage && item.parse === "done" && item.embed === "pending";
  // No Retry where it can't help: a failed download isn't on disk (the next
  // sync fetches it), a non-retryable error never clears, and a latching one
  // holds every file until its cause is fixed.
  const retryable =
    s.phase === "failed" &&
    item.download !== "error" &&
    item.errorRetryable !== false &&
    !item.errorLatching;
  let run: RowActions["run"];
  // Already queued: the embed queue dedups, so ▶ would do nothing.
  if (canRun && item.embed !== "queued") {
    if (s.phase === "skipped") run = { label: "Parse now", hint: "Parse this file now" };
    else if (retryable) run = { label: "Retry", hint: "Try this file again" };
    else if (s.phase === "paused") run = { label: "Resume", hint: "Resume where it left off" };
    else if (embedNow) run = { label: "Embed", hint: "Embed this file now" };
  }
  // Anything short of a finished parse, once the bytes are on disk. A
  // spreadsheet's conversion takes moments and bills nothing, so it is not
  // skipped.
  const skip =
    canSkip &&
    !isSheetFile(item.filename) &&
    item.download === "done" &&
    (item.parse === "pending" ||
      item.parse === "queued" ||
      item.parse === "active" ||
      item.parse === "error");
  return { run, skip };
}

/** Opens the file beside the page; through its row when there is one, so
 *  the visit is recorded like any file row's. */
export function openItem(item: PipelineItem) {
  const beside = () => openBeside(filePagePath(item.subjectId, item.relativePath));
  getFileByRelativePath(item.relativePath)
    .then((file) => (file ? openFileSmart(file) : beside()))
    .catch(beside);
}
