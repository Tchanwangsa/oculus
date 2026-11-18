import { getFileByRelativePath } from "@/lib/db";
import { filePagePath, openFileSmart } from "@/lib/files/openFile";
import { openBeside } from "@/lib/shell/tabRouters";
import type { PipelineItem } from "@/stores/sync/pipelineStore";

/** Opens the file beside the page; through its row when there is one, so
 *  the visit is recorded like any file row's. */
export function openItem(item: PipelineItem) {
  const beside = () => openBeside(filePagePath(item.subjectId, item.relativePath));
  getFileByRelativePath(item.relativePath)
    .then((file) => (file ? openFileSmart(file) : beside()))
    .catch(beside);
}
