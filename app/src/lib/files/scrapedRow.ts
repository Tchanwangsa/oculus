import { getFileByRelativePath, type DbFile } from "@/lib/db";
import { FILE_SCRAPED_EVENT } from "@/lib/pipeline/syncRunner";

/**
 * The `files` row `useBackendEvents` writes from a download's `scrape-file`
 * event. The command can resolve before that write lands, so wait for its
 * FILE_SCRAPED_EVENT; the listener goes on first so the event can't slip by.
 */
export function scrapedRow(relativePath: string): Promise<DbFile | null> {
  return new Promise((resolve) => {
    let settled = false;
    const finish = (file: DbFile | null) => {
      if (settled) return;
      settled = true;
      window.removeEventListener(FILE_SCRAPED_EVENT, check);
      clearTimeout(fallback);
      resolve(file);
    };
    const check = () => {
      getFileByRelativePath(relativePath)
        .then((file) => { if (file) finish(file); })
        .catch(() => {});
    };
    window.addEventListener(FILE_SCRAPED_EVENT, check);
    // A row write that failed never raises the event; stop waiting for it.
    const fallback = setTimeout(() => {
      getFileByRelativePath(relativePath).then(finish, () => finish(null));
    }, 10_000);
    check();
  });
}
