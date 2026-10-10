import { useStoredState } from "@/hooks/ui/useStoredState";
import { useTabActive } from "@/components/tabs/TabContext";
import { useMemo, useState } from "react";
import { WarningCircle, XCircle, Broom, Play } from "@phosphor-icons/react";
import { cn } from "@/lib/utils";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Progress } from "@/components/ui/progress";
import { ViewTabs } from "@/components/ui/table/ViewTabs";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { fmtAgo, sqliteUtcToMs } from "@/lib/format/format";
import { useAuth } from "@/hooks/sync/useAuth";
import { useSyncStore } from "@/stores/sync/syncStore";
import { usePipelineStore } from "@/stores/sync/pipelineStore";
import { PipelineTable } from "@/components/sync/PipelineTable";
import { ResultCertWarning } from "@/components/sync/ResultCertWarning";
import { SyncHistoryTable } from "@/components/sync/SyncHistoryTable";
import { SubjectPicker } from "@/components/sync/SubjectPicker";
import { SyncSettings } from "@/components/sync/SyncSettings";
import { PHASE_LABEL, VIEWS, VIEW_KEY, type ActivityView } from "./sync/constants";
import { countFinished, countPhases } from "./sync/derive";
import { useSyncActions } from "./sync/useSyncActions";
import { useSyncBoot } from "./sync/useSyncBoot";
import { useSyncEvents } from "./sync/useSyncEvents";

export default function SyncPage() {
  const { status: authStatus, connect } = useAuth();
  const active = useTabActive();
  const [loadingSubjects, setLoadingSubjects] = useState(false);
  const [subjectsError, setSubjectsError] = useState<string | null>(null);
  const [view, setView] = useStoredState<ActivityView>(VIEW_KEY, (stored) =>
    stored === "pipeline" ? "pipeline" : "history",
  );

  // Sync progress lives in the global store (survives navigation).
  const scraping = useSyncStore((s) => s.scraping);
  const progress = useSyncStore((s) => s.progress);
  const completedAt = useSyncStore((s) => s.completedAt);
  const syncError = useSyncStore((s) => s.error);

  // Pipeline (per-file download → parse tracking).
  const pipelineItems = usePipelineStore((s) => s.items);
  const embedStage = usePipelineStore((s) => s.embedStage);
  const clearFinished = usePipelineStore((s) => s.clearFinished);

  const { subjects, selectedIds, setSelectedIds, runs, setRuns, loadFromDb, requestSeed, activeRef } =
    useSyncBoot(active, scraping);

  const items = useMemo(() => Object.values(pipelineItems), [pipelineItems]);
  const counts = useMemo(() => countPhases(items, embedStage), [items, embedStage]);

  useSyncEvents({
    loadFromDb,
    requestSeed,
    activeRef,
    completedAt,
    syncError,
    setLoadingSubjects,
    setSubjectsError,
  });

  const {
    handleRefetchSubjects,
    handleCancel,
    handleSyncClick,
    resumeItem,
    skipItem,
    resumeAll,
    toggleSubject,
  } = useSyncActions({
    authStatus,
    connect,
    selectedIds,
    setSelectedIds,
    setRuns,
    setView,
    setLoadingSubjects,
    setSubjectsError,
  });

  const phase = progress?.phase ? (PHASE_LABEL[progress.phase] ?? progress.phase) : null;
  const finishedCount = countFinished(items, embedStage);
  const lastCompleted = runs.find((r) => r.status === "completed" && r.finished_at);
  const needsAuth = authStatus !== "connected";

  return (
    <div className="flex flex-col h-full">
      <div className="shrink-0 flex items-end border-b border-border-subtle px-5 pt-4">
        <ViewTabs tabs={VIEWS} value={view} onChange={(v) => setView(v)} />
      </div>

      {/* Fixed height so switching views doesn't jump the toolbar. */}
      <div className="shrink-0 flex h-12 items-center gap-2 px-5">
        {view === "history" ? (
          <>
            <SubjectPicker
              subjects={subjects}
              selectedIds={selectedIds}
              onToggle={toggleSubject}
              onRefetch={handleRefetchSubjects}
              refetching={loadingSubjects}
              canRefetch={authStatus === "connected"}
            />

            <SyncSettings />

            <span className="flex-1" />

            {scraping && progress ? (
              <div className="flex items-center gap-2.5 min-w-0">
                <span className="text-[11px] text-muted-foreground truncate max-w-64">
                  {progress.course
                    ? `${progress.course}${phase && progress.phase !== "complete" ? ` — ${phase}` : ""}`
                    : "Starting…"}
                </span>
                <Progress
                  value={progress.total ? (progress.done / progress.total) * 100 : 0}
                  className="h-1 w-24"
                />
                <span className="text-[11px] text-muted-foreground tabular-nums shrink-0">
                  {progress.done}/{progress.total}
                </span>
              </div>
            ) : (
              <span className="text-[11px] text-muted-foreground shrink-0">
                Last synced{" "}
                {lastCompleted ? fmtAgo(sqliteUtcToMs(lastCompleted.finished_at)) : "never"}
              </span>
            )}

            {scraping ? (
              <Button
                variant="outline"
                size="sm"
                onClick={handleCancel}
                className="h-7 shrink-0 text-destructive hover:text-destructive hover:bg-destructive/10 border-destructive/30"
              >
                <XCircle size={13} /> Cancel
              </Button>
            ) : needsAuth || selectedIds.size === 0 ? (
              <Tooltip>
                {/* A disabled button swallows pointer events; the span doesn't. */}
                <TooltipTrigger asChild>
                  <span className="shrink-0">
                    <Button
                      size="sm"
                      className={cn("h-7", needsAuth && "opacity-50")}
                      onClick={handleSyncClick}
                      disabled={selectedIds.size === 0}
                    >
                      Sync now
                    </Button>
                  </span>
                </TooltipTrigger>
                <TooltipContent>
                  {needsAuth
                    ? authStatus === "expired"
                      ? "Canvas session expired — click to sign in again"
                      : "Not connected to Canvas — click to sign in"
                    : "Select at least one subject first"}
                </TooltipContent>
              </Tooltip>
            ) : (
              <Button size="sm" className="h-7 shrink-0" onClick={handleSyncClick}>
                Sync now
              </Button>
            )}
          </>
        ) : (
          <>
            <div className="flex items-center gap-1.5">
              {counts.active > 0 && (
                <Badge className="text-[11px]">{counts.active} running</Badge>
              )}
              {counts.waiting > 0 && (
                <Badge variant="secondary" className="text-[11px]">
                  {counts.waiting} waiting
                </Badge>
              )}
              {counts.paused > 0 && (
                <Badge variant="warning" className="text-[11px]">
                  {counts.paused} paused
                </Badge>
              )}
              {counts.failed > 0 && (
                <Badge variant="destructive" className="text-[11px]">
                  {counts.failed} failed
                </Badge>
              )}
              {counts.skipped > 0 && (
                <Badge variant="outline" className="text-[11px] text-muted-foreground">
                  {counts.skipped} skipped
                </Badge>
              )}
              {counts.done > 0 && (
                <Badge variant="success" className="text-[11px]">
                  {counts.done} done
                </Badge>
              )}
              {items.length === 0 && (
                <span className="text-[11px] text-muted-foreground">
                  Nothing parsed yet
                </span>
              )}
            </div>

            <span className="flex-1" />

            <ResultCertWarning />

            {counts.paused > 0 && (
              <Button
                variant="ghost"
                size="sm"
                onClick={resumeAll}
                className="h-7 text-xs text-muted-foreground hover:text-foreground"
              >
                <Play size={13} /> Resume all ({counts.paused})
              </Button>
            )}
            {finishedCount > 0 && (
              <Button
                variant="ghost"
                size="sm"
                onClick={clearFinished}
                className="h-7 text-xs text-muted-foreground hover:text-foreground"
              >
                <Broom size={13} /> Clear finished
              </Button>
            )}
          </>
        )}
      </div>

      {subjectsError && (
        <div className="shrink-0 px-5 pb-2.5">
          <Alert variant="destructive" className="px-3 py-2.5">
            <WarningCircle />
            <AlertDescription className="text-xs">{subjectsError}</AlertDescription>
          </Alert>
        </div>
      )}

      <div className="flex-1 min-h-0">
        {view === "history" ? (
          <SyncHistoryTable runs={runs} progress={progress} />
        ) : (
          <PipelineTable items={items} onResume={resumeItem} onSkip={skipItem} />
        )}
      </div>
    </div>
  );
}
