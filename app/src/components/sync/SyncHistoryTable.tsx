import { useEffect, useState } from "react";
import { CaretRight, File as FileIcon, FilePdf, FileDoc } from "@phosphor-icons/react";
import { cn } from "@/lib/utils";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Progress } from "@/components/ui/progress";
import {
  getSyncRunFiles,
  INTERRUPTED_SYNC_ERROR,
  type SyncFileAction,
  type SyncRunFile,
  type SyncRunSummary,
} from "@/lib/db";
import {
  displayCode,
  fmtClock,
  fmtDayHeading,
  fmtDuration,
  fmtSize,
  sqliteUtcToMs,
} from "@/lib/format";
import { SubjectIcon } from "@/components/subjects/SubjectIcon";
import {
  TablePagination,
  usePagedRows,
} from "@/components/ui/TablePagination";
import type { SyncProgress } from "@/stores/syncStore";

/**
 * One row per sync run, newest first. A row expands into the run's changed
 * files inline; the full ledger (including everything skipped) lives in a
 * modal so a routine "nothing changed" run stays one quiet line.
 */

const ACTION_LABEL: Record<SyncFileAction, string> = {
  new: "Downloaded",
  updated: "Updated",
  unchanged: "Skipped",
};

const ACTION_VARIANT: Record<SyncFileAction, "success" | "default" | "secondary"> = {
  new: "success",
  updated: "default",
  unchanged: "secondary",
};

/** Column template shared by the header and every row. */
// The run name is a fixed-length string ("Manual run at 9 Sep, 16:59"), so it
// gets a fixed column rather than the leftover space — the file counts are the
// column that actually has something to say, and they take the slack instead.
const COLS =
  "grid grid-cols-[14px_200px_90px_80px_minmax(0,1fr)_100px] items-center gap-3 px-5";

const INLINE_FILE_LIMIT = 8;

/** Codes the run targeted — from the stored list, or (for runs recorded
 *  before that existed) whatever subjects its file ledger touched. */
function runSubjectCodes(run: SyncRunSummary, files: SyncRunFile[]): string[] {
  if (run.subject_codes) {
    try {
      const parsed = JSON.parse(run.subject_codes);
      if (Array.isArray(parsed) && parsed.length > 0) return parsed;
    } catch {
      /* fall through to the ledger */
    }
  }
  return [...new Set(files.map((f) => f.subject_code).filter((c): c is string => !!c))];
}

function SubjectChips({ codes }: { codes: string[] }) {
  if (codes.length === 0) return null;
  return (
    <div className="flex flex-wrap items-center gap-1.5 py-1.5">
      {codes.map((code) => (
        <span
          key={code}
          className="inline-flex items-center gap-1.5 rounded-md border border-border-subtle bg-surface px-2 py-1 text-[11px] text-foreground"
        >
          <SubjectIcon code={code} size={11} />
          {displayCode(code)}
        </span>
      ))}
    </div>
  );
}

function fileIcon(path: string) {
  const lower = path.toLowerCase();
  if (lower.endsWith(".pdf")) return FilePdf;
  if (/\.(pptx?|docx?|xlsx?)$/.test(lower)) return FileDoc;
  return FileIcon;
}

function FileLine({ file }: { file: SyncRunFile }) {
  const Icon = fileIcon(file.relative_path);
  const name = file.relative_path.split("/").pop() ?? file.relative_path;
  return (
    <div className="flex items-center gap-2.5 py-1.5 min-w-0">
      <Icon size={13} className="shrink-0 text-muted-foreground/70" />
      <span className="text-xs text-foreground truncate">{name}</span>
      <span className="text-[11px] text-muted-foreground shrink-0">
        {file.subject_code ? displayCode(file.subject_code) : ""}
      </span>
      <span className="flex-1" />
      <span className="text-[11px] text-muted-foreground tabular-nums shrink-0">
        {fmtSize(file.size_bytes)}
      </span>
      <Badge variant={ACTION_VARIANT[file.action]} className="text-[11px] shrink-0">
        {ACTION_LABEL[file.action]}
      </Badge>
    </div>
  );
}

// ── Counts ────────────────────────────────────────────────────────────────────

function FileCounts({ run }: { run: SyncRunSummary }) {
  if (run.file_count === 0) {
    return <span className="text-[11px] text-muted-foreground/60">—</span>;
  }
  const parts: Array<{ n: number; label: string; cls: string }> = [
    { n: run.new_count, label: "downloaded", cls: "text-success" },
    { n: run.updated_count, label: "updated", cls: "text-brand" },
    { n: run.unchanged_count, label: "skipped", cls: "text-muted-foreground" },
  ];
  return (
    <span className="text-[11px] text-muted-foreground truncate">
      {parts
        .filter((p) => p.n > 0)
        .map((p, i) => (
          <span key={p.label}>
            {i > 0 && <span className="text-muted-foreground/40"> · </span>}
            <span className={cn("tabular-nums", p.cls)}>{p.n}</span> {p.label}
          </span>
        ))}
    </span>
  );
}

// ── Files modal ───────────────────────────────────────────────────────────────

function RunFilesDialog({
  run,
  files,
  open,
  onOpenChange,
}: {
  run: SyncRunSummary;
  files: SyncRunFile[];
  open: boolean;
  onOpenChange: (v: boolean) => void;
}) {
  const groups = (["new", "updated", "unchanged"] as const)
    .map((action) => ({ action, files: files.filter((f) => f.action === action) }))
    .filter((g) => g.files.length > 0);

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-2xl">
        <DialogHeader>
          <DialogTitle className="text-sm">
            Files — {fmtClock(sqliteUtcToMs(run.started_at), true)}
          </DialogTitle>
          <DialogDescription className="text-xs">
            {files.length} file{files.length === 1 ? "" : "s"} touched in this run
          </DialogDescription>
        </DialogHeader>
        <div className="max-h-[55vh] overflow-y-auto -mx-1 px-1">
          {groups.map((g) => (
            <div key={g.action} className="mb-3 last:mb-0">
              <p className="text-[11px] font-medium text-muted-foreground py-1.5 sticky top-0 bg-background">
                {ACTION_LABEL[g.action]} ({g.files.length})
              </p>
              <div className="divide-y divide-border-subtle">
                {g.files.map((f) => (
                  <FileLine key={f.id} file={f} />
                ))}
              </div>
            </div>
          ))}
        </div>
      </DialogContent>
    </Dialog>
  );
}

// ── Expanded run detail ───────────────────────────────────────────────────────

function RunDetail({ run }: { run: SyncRunSummary }) {
  const [files, setFiles] = useState<SyncRunFile[] | null>(null);
  const [modalOpen, setModalOpen] = useState(false);

  useEffect(() => {
    let alive = true;
    getSyncRunFiles(run.id)
      .then((rows) => alive && setFiles(rows))
      .catch(() => alive && setFiles([]));
    return () => {
      alive = false;
    };
    // Refetch while the run is live so the list grows with the sync.
  }, [run.id, run.file_count, run.status]);

  if (files === null) {
    return <div className="px-12 py-3 text-xs text-muted-foreground">Loading…</div>;
  }

  const changed = files.filter((f) => f.action !== "unchanged");
  const shown = changed.slice(0, INLINE_FILE_LIMIT);
  const skipped = run.unchanged_count;

  return (
    <div className="px-12 pb-3 pt-1">
      {run.error && (
        <p className="text-xs text-destructive py-1.5">{run.error}</p>
      )}

      <SubjectChips codes={runSubjectCodes(run, files)} />

      {files.length === 0 ? (
        <p className="text-xs text-muted-foreground py-1.5">
          {run.status === "running"
            ? "Nothing touched yet…"
            : "No per-file records for this run."}
        </p>
      ) : (
        <>
          {shown.length > 0 ? (
            <div className="divide-y divide-border-subtle">
              {shown.map((f) => (
                <FileLine key={f.id} file={f} />
              ))}
            </div>
          ) : (
            <p className="text-xs text-muted-foreground py-1.5">
              Nothing new — every file was already up to date.
            </p>
          )}

          <div className="flex items-center gap-3 pt-2">
            {changed.length > shown.length && (
              <span className="text-[11px] text-muted-foreground">
                +{changed.length - shown.length} more changed
              </span>
            )}
            {skipped > 0 && shown.length > 0 && (
              <span className="text-[11px] text-muted-foreground">
                {skipped} unchanged file{skipped === 1 ? "" : "s"} skipped
              </span>
            )}
            <Button
              variant="ghost"
              size="sm"
              onClick={() => setModalOpen(true)}
              className="h-6 px-2 text-[11px] text-muted-foreground hover:text-foreground -ml-2"
            >
              View all {files.length} files
            </Button>
          </div>

          <RunFilesDialog run={run} files={files} open={modalOpen} onOpenChange={setModalOpen} />
        </>
      )}
    </div>
  );
}

// ── Run rows ──────────────────────────────────────────────────────────────────

function RunRow({
  run,
  progress,
  expanded,
  onToggle,
}: {
  run: SyncRunSummary;
  /** Live progress, only for the run currently scraping. */
  progress: SyncProgress | null;
  expanded: boolean;
  onToggle: () => void;
}) {
  const startedMs = sqliteUtcToMs(run.started_at);
  const finishedMs = sqliteUtcToMs(run.finished_at);
  const running = run.status === "running";
  // Reconciled runs get finished_at stamped at the NEXT app launch, so the
  // elapsed time is a fiction — show nothing rather than a made-up duration.
  const interrupted = run.error === INTERRUPTED_SYNC_ERROR;

  return (
    <div>
      <div
        role="button"
        tabIndex={0}
        onClick={onToggle}
        onKeyDown={(e) => e.key === "Enter" && onToggle()}
        className={cn(COLS, "py-2.5 cursor-pointer hover:bg-surface/60 transition-colors")}
      >
        <CaretRight
          size={9}
          className={cn(
            "shrink-0 text-muted-foreground/50 transition-transform",
            expanded && "rotate-90",
          )}
        />

        <span className="text-xs text-foreground truncate">
          {run.origin === "scheduled" ? "Scheduled" : "Manual"} run at{" "}
          <span className="tabular-nums">{fmtClock(startedMs, true)}</span>
        </span>

        <span className="text-[11px] text-muted-foreground tabular-nums">
          {running ? "running" : interrupted ? "—" : fmtDuration(startedMs, finishedMs)}
        </span>

        <span className="text-[11px] text-muted-foreground tabular-nums">
          {run.subjects_synced > 0
            ? `${run.subjects_synced} subject${run.subjects_synced === 1 ? "" : "s"}`
            : "—"}
        </span>

        <FileCounts run={run} />

        <div className="justify-self-end flex items-center gap-2 min-w-0">
          {running && progress ? (
            <div className="flex items-center gap-2">
              <Progress
                value={progress.total ? (progress.done / progress.total) * 100 : 0}
                className="h-1 w-16"
              />
              <Badge className="text-[11px]">Running</Badge>
            </div>
          ) : (
            <Badge
              variant={
                run.status === "completed"
                  ? "success"
                  : interrupted
                    ? "warning"
                    : run.status === "failed"
                      ? "destructive"
                      : "default"
              }
              className="text-[11px]"
            >
              {run.status === "completed"
                ? "Completed"
                : interrupted
                  ? "Interrupted"
                  : run.status === "failed"
                    ? "Failed"
                    : "Running"}
            </Badge>
          )}
        </div>
      </div>

      {expanded && <RunDetail run={run} />}
    </div>
  );
}

/** Consecutive runs bucketed by local calendar day; input is newest-first so
 *  each day's runs stay contiguous. */
function groupByDay(runs: SyncRunSummary[]) {
  const groups: Array<{ key: string; heading: string; runs: SyncRunSummary[] }> = [];
  for (const run of runs) {
    const ms = sqliteUtcToMs(run.started_at) ?? 0;
    const d = new Date(ms);
    const key = `${d.getFullYear()}-${d.getMonth()}-${d.getDate()}`;
    const last = groups[groups.length - 1];
    if (last?.key === key) last.runs.push(run);
    else groups.push({ key, heading: fmtDayHeading(ms), runs: [run] });
  }
  return groups;
}

/** Runs per page. Enough that a normal week fits on page one, few enough
 *  that the day groups stay scannable. */
const PAGE_SIZE = 25;

export function SyncHistoryTable({
  runs,
  progress,
}: {
  runs: SyncRunSummary[];
  progress: SyncProgress | null;
}) {
  const [expanded, setExpanded] = useState<Set<number>>(new Set());
  const [collapsedDays, setCollapsedDays] = useState<Set<string>>(new Set());
  const { page, pageCount, setPage, pageRows } = usePagedRows(runs, PAGE_SIZE);

  const toggle = (id: number) =>
    setExpanded((prev) => {
      const next = new Set(prev);
      next.has(id) ? next.delete(id) : next.add(id);
      return next;
    });

  const toggleDay = (key: string) =>
    setCollapsedDays((prev) => {
      const next = new Set(prev);
      next.has(key) ? next.delete(key) : next.add(key);
      return next;
    });

  return (
    <div className="flex h-full flex-col">
      {/* The header sits OUTSIDE the scroll container, not sticky inside it:
          the scrollbar is a 6px classic bar that takes its gutter from the
          scroller's full height, so a header within it gets a bar drawn down
          its right edge. The wrapper's `pr-1.5` re-creates that gutter's width
          for the header, and `scrollbar-gutter: stable` on the body keeps the
          gutter reserved when there is nothing to scroll — without both, the
          header and its rows would sit 6px out of column. */}
      <div className="shrink-0 pr-1.5">
        <div className={cn(COLS, "border-b border-border-subtle bg-card py-2")}>
          <span />
          {["Started", "Duration", "Subjects", "Files", "Status"].map((h, i) => (
            <span
              key={h}
              className={cn(
                "text-[11px] font-medium text-muted-foreground",
                i === 4 && "justify-self-end",
              )}
            >
              {h}
            </span>
          ))}
        </div>
      </div>

      {/* The body scrolls between the fixed header and the pinned footer. */}
      <div className="flex-1 min-h-0 overflow-y-auto [scrollbar-gutter:stable]">
        {runs.length === 0 && (
          <p className="px-5 py-16 text-center text-xs text-muted-foreground">
            No sync runs yet — pick your subjects above and run one.
          </p>
        )}

        <div className="divide-y divide-border-subtle">
          {groupByDay(pageRows).map((group) => {
            const collapsed = collapsedDays.has(group.key);
            return (
              <div key={group.key}>
                <div
                  role="button"
                  tabIndex={0}
                  onClick={() => toggleDay(group.key)}
                  onKeyDown={(e) => e.key === "Enter" && toggleDay(group.key)}
                  className={cn(
                    "flex items-center gap-2 px-5 py-1.5 bg-surface/70 cursor-pointer select-none hover:bg-surface transition-colors",
                    !collapsed && "border-b border-border-subtle",
                  )}
                >
                  <CaretRight
                    size={9}
                    className={cn(
                      "shrink-0 text-muted-foreground/50 transition-transform",
                      !collapsed && "rotate-90",
                    )}
                  />
                  <span className="text-[11px] font-medium text-muted-foreground">
                    {group.heading}
                  </span>
                  {collapsed && (
                    <span className="flex h-4 min-w-4 items-center justify-center rounded-full bg-muted-foreground/15 px-1 text-[10px] font-medium tabular-nums text-muted-foreground">
                      {group.runs.length}
                    </span>
                  )}
                </div>
                {!collapsed && (
                  <div className="divide-y divide-border-subtle">
                    {group.runs.map((run) => (
                      <RunRow
                        key={run.id}
                        run={run}
                        progress={run.status === "running" ? progress : null}
                        expanded={expanded.has(run.id)}
                        onToggle={() => toggle(run.id)}
                      />
                    ))}
                  </div>
                )}
              </div>
            );
          })}
        </div>
      </div>

      <TablePagination
        page={page}
        pageCount={pageCount}
        onPage={setPage}
        total={runs.length}
        unit="sync run"
      />
    </div>
  );
}
