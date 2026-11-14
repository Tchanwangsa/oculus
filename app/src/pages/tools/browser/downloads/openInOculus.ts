import type { SavedTo } from "@/lib/browser/downloads/types";
import { getFileByRelativePath } from "@/lib/db";
import { openFileSmart } from "@/lib/files/openFile";

/** Opens a download saved in Oculus as any list opens a library file: its
 *  page beside the browser, or the system viewer for a type with none. */
export async function openInOculus(saved: Extract<SavedTo, { kind: "oculus" }>): Promise<void> {
  const path = saved.outcome.file?.relative_path;
  if (!path) throw new Error("it did not land in the library");
  const file = await getFileByRelativePath(path);
  if (!file) throw new Error("it is not in the library yet");
  openFileSmart(file);
}
