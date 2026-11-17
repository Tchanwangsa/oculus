import { getSubjectCompletedThreads } from "@/lib/harness/completedReads";
import type { HarnessThread } from "@/lib/harness/types";
import type { DbFile } from "@/lib/db";
import { parseEdThread, type EdThread } from "@/lib/ed/threads";
import { parseTaskDoc, type TaskDoc } from "@/pages/subject/content/assignments/grouping";

export interface FileMetadata {
  thread: EdThread | null;
  task: TaskDoc | null;
}

export function parseOverviewFile(markdown: string, file: DbFile): FileMetadata {
  return {
    thread: file.category === "ed" ? parseEdThread(markdown, file) : null,
    task: file.category === "assignment" || file.category === "quiz" ? parseTaskDoc(markdown, file) : null,
  };
}

/** Read every page: the selected week may contain chats beyond the first fold. */
export async function loadCompletedThreads(
  subjectId: number,
  read: typeof getSubjectCompletedThreads = getSubjectCompletedThreads,
): Promise<HarnessThread[]> {
  const threads = new Map<number, HarnessThread>();
  let offset: number | null = 0;
  while (offset != null) {
    const page = await read(subjectId, offset, 100);
    for (const thread of page.threads) threads.set(thread.id, thread);
    offset = page.nextOffset != null && page.nextOffset > offset ? page.nextOffset : null;
  }
  return [...threads.values()];
}
