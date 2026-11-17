import { useCallback, useEffect, useRef, useState } from "react";

import { fileKindTone } from "@/components/drive/content/ItemIcon";
import { RecencyDot } from "@/components/files/FileRecency";
import { SubjectIcon } from "@/components/subjects/SubjectIcon";
import { PageHeader, PaneTitle, PaneTrail } from "@/components/tabs/PaneHeader";
import { SideNavCollapseToggle } from "@/components/ui/layout/SideNav";
import { SkeletonRows } from "@/components/ui/layout/PageParts";
import { GridTable, HeaderLabels } from "@/components/ui/table/GridTable";
import { useWindowEvent } from "@/hooks/backend/useEvents";
import { LIBRARY_CHANGE_EVENTS } from "@/hooks/drive/useFolderListing";
import { useElementWidth } from "@/hooks/drive/useElementWidth";
import { useDrivePanel } from "@/layouts/DriveLayout";
import { relativeTime } from "@/lib/activity/recents";
import { getRecentDriveFiles, type RecentDriveFile } from "@/lib/db";
import { parentRel } from "@/lib/drive/paths";
import { categoryIconFor } from "@/lib/files/fileTypes";
import { courseRelOf } from "@/lib/files/libraryLayout";
import { FILE_ACCESSED_EVENT, filePagePath, fileTitle, openFileSmart } from "@/lib/files/openFile";
import { displayCode, fmtSize, sqliteUtcToMs } from "@/lib/format/format";
import { cn } from "@/lib/utils";

const COLS = "grid grid-cols-[minmax(0,1fr)_minmax(0,220px)_120px_72px] items-center gap-3 px-5";
const NARROW_COLS = "grid grid-cols-[minmax(0,1fr)_96px] items-center gap-3 px-5";
/** Below this width the list keeps Name and the time. */
const NARROW = 560;
/** Reads of the list wait for a burst of library events to stop. */
const SETTLE_MS = 300;

/**
 * Drive's Recent: the files the student opened, and their own files they
 * made or changed, across every subject, newest first
 * (`getRecentDriveFiles`). A row opens its file as Drive does
 * (`openFileSmart`), beside the page.
 */
export default function DriveRecentPage() {
  const panel = useDrivePanel();
  const ref = useRef<HTMLDivElement>(null);
  const width = useElementWidth(ref);
  const narrow = width > 0 && width < NARROW;
  const [files, setFiles] = useState<RecentDriveFile[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(() => {
    getRecentDriveFiles()
      .then((rows) => {
        setFiles(rows);
        setError(null);
      })
      .catch((e) => setError(String(e)));
  }, []);
  useEffect(load, [load]);

  const settle = useRef<ReturnType<typeof setTimeout> | null>(null);
  useEffect(() => () => { if (settle.current) clearTimeout(settle.current); }, []);
  useWindowEvent([...LIBRARY_CHANGE_EVENTS, FILE_ACCESSED_EVENT], () => {
    if (settle.current) clearTimeout(settle.current);
    settle.current = setTimeout(() => { settle.current = null; load(); }, SETTLE_MS);
  });

  const cols = narrow ? NARROW_COLS : COLS;
  const labels = narrow ? ["Name", "When"] : ["Name", "Location", "When", "Size"];
  const empty = files == null
    ? error
      ? <p className="px-5 py-16 text-center text-xs text-muted-foreground">{error}</p>
      : <SkeletonRows count={6} className="mx-5 mt-3" />
    : files.length === 0
      ? <p className="px-5 py-16 text-center text-xs text-muted-foreground">Files you open or change show here.</p>
      : undefined;

  return (
    <div ref={ref} className="flex h-full min-h-0 flex-col">
      <PageHeader>
        <SideNavCollapseToggle panel={panel} />
        <PaneTrail>
          <PaneTitle>Recent</PaneTitle>
        </PaneTrail>
      </PageHeader>
      <div className="min-h-0 flex-1">
        <GridTable cols={cols} header={<HeaderLabels labels={labels} />} empty={empty}>
          <div className="divide-y divide-border-subtle">
            {files?.map((file) => <RecentRow key={file.id} file={file} cols={cols} narrow={narrow} />)}
          </div>
        </GridTable>
      </div>
    </div>
  );
}

function RecentRow({ file, cols, narrow }: { file: RecentDriveFile; cols: string; narrow: boolean }) {
  const Icon = categoryIconFor(file);
  const folder = parentRel(courseRelOf(file.relative_path));
  const ms = sqliteUtcToMs(file.recent_at);
  const when = ms == null ? "" : relativeTime(ms);
  return (
    <button
      type="button"
      data-tab-href={filePagePath(file.subject_id, file.relative_path)}
      onClick={() => openFileSmart(file)}
      title={file.relative_path}
      className={cn(cols, "w-full cursor-pointer py-2 text-left outline-none transition-[background-color] duration-100 hover:bg-surface/60 focus-visible:bg-surface/60")}
    >
      <div className="flex min-w-0 items-center gap-2.5">
        <Icon size={15} weight={file.filename.toLowerCase().endsWith(".pdf") ? "fill" : "regular"} className={cn("shrink-0", fileKindTone(file.filename, file.category))} aria-hidden />
        <RecencyDot file={file} />
        <span className="min-w-0 truncate text-[12.5px] text-foreground">{fileTitle(file)}</span>
      </div>
      {!narrow && (
        <div className="flex min-w-0 items-center gap-1.5 text-[11.5px] text-muted-foreground">
          <SubjectIcon code={file.subject_code} size={12} />
          <span className="shrink-0">{displayCode(file.subject_code)}</span>
          {folder && <span className="min-w-0 truncate">· {folder}</span>}
        </div>
      )}
      <span className="text-right text-[11.5px] text-muted-foreground tabular-nums">
        {narrow ? when : `${file.recent_why === "opened" ? "Opened" : "Changed"} ${when}`}
      </span>
      {!narrow && (
        <span className="text-right text-[11.5px] text-muted-foreground tabular-nums">{fmtSize(file.size_bytes)}</span>
      )}
    </button>
  );
}
