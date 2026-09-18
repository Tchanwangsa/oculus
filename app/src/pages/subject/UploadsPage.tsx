import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { invoke } from "@tauri-apps/api/core";
import { CircleNotch, Trash, UploadSimple, Warning } from "@phosphor-icons/react";

import { cn } from "@/lib/utils";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Skeleton } from "@/components/ui/skeleton";
import { useSubject } from "@/layouts/SubjectLayout";
import { useSubjectFiles } from "@/hooks/useSubjectFiles";
import { useTabActive } from "@/components/tabs/TabContext";
import { useParseStore } from "@/stores/parseStore";
import { setParseStatusByPath, type DbFile } from "@/lib/db";
import { fileIconFor, isPdfBacked } from "@/lib/fileTypes";
import { fmtSize } from "@/lib/format";
import { openFileSmart } from "@/lib/openFile";
import {
  UPLOADS_CHANGED_EVENT,
  addUploads,
  pickUploads,
  removeUpload,
} from "@/lib/uploads";

/**
 * A subject's own files — the tutor's handout, last year's exam, a friend's
 * notes: material that belongs to the subject but was never on Canvas.
 *
 * The page is thin on purpose. Once Rust has copied the bytes into
 * `courses/<code>/uploads/`, an upload is an ordinary library file, so nothing
 * here re-implements parsing, conversion, badges or opening — it reuses the
 * Downloads row's shapes and lets the pipeline do the rest.
 */
export default function SubjectUploadsPage() {
  const subject = useSubject();
  const tabActive = useTabActive();
  const { byCategory, loading, reload } = useSubjectFiles(subject.id);
  const uploads = byCategory.upload;

  /** Names currently being copied and converted. LibreOffice takes a couple of
   *  seconds per document, and a picker that closes onto an unchanged list
   *  reads as a click that did nothing. */
  const [importing, setImporting] = useState<string[]>([]);
  /** Per-file problems from the last add, kept until the next one. */
  const [problems, setProblems] = useState<string[]>([]);
  const [pendingDelete, setPendingDelete] = useState<DbFile | null>(null);
  const [deleting, setDeleting] = useState(false);
  /** True while the pointer is over the window holding files from Finder. */
  const [dropping, setDropping] = useState(false);

  const liveStatuses = useParseStore((s) => s.statuses);
  const mergeParseStatuses = useParseStore((s) => s.merge);

  useEffect(() => {
    window.addEventListener(UPLOADS_CHANGED_EVENT, reload);
    return () => window.removeEventListener(UPLOADS_CHANGED_EVENT, reload);
  }, [reload]);

  // Same reconciliation Downloads does: a file parsed in an earlier session has
  // its artifacts on disk and nothing in this session's store.
  useEffect(() => {
    const paths = uploads.filter((f) => isPdfBacked(f.filename)).map((f) => f.relative_path);
    if (paths.length === 0) return;
    invoke<Array<[string, string]>>("scan_parsed_files", { relativePaths: paths })
      .then((entries) => {
        if (entries.length === 0) return;
        mergeParseStatuses(Object.fromEntries(entries));
        setParseStatusByPath(entries).catch(() => {});
      })
      .catch(() => {});
  }, [uploads, mergeParseStatuses]);

  const add = useCallback(
    async (paths: string[]) => {
      if (paths.length === 0) return;
      setProblems([]);
      setImporting(paths.map((p) => p.split("/").pop() ?? p));
      try {
        const outcomes = await addUploads(subject, paths);
        setProblems(
          outcomes
            .filter((o) => o.error)
            .map((o) => `${o.source}: ${o.error}`),
        );
      } catch (e) {
        setProblems([String(e)]);
      } finally {
        setImporting([]);
      }
    },
    [subject],
  );

  const choose = useCallback(async () => {
    try {
      await add(await pickUploads());
    } catch (e) {
      setProblems([String(e)]);
    }
  }, [add]);

  // Files dragged from Finder arrive as a native window event carrying paths,
  // not as an HTML drop — WebKit never sees them. The event is the *window's*,
  // so a backgrounded copy of this page would claim a drop meant for whatever
  // is in front: `tabActive` is what scopes it to the tab being looked at.
  const addRef = useRef(add);
  addRef.current = add;
  useEffect(() => {
    if (!tabActive) return;
    let cancelled = false;
    const un = getCurrentWebview().onDragDropEvent((event) => {
      if (cancelled) return;
      if (event.payload.type === "over") setDropping(true);
      else if (event.payload.type === "leave") setDropping(false);
      else if (event.payload.type === "drop") {
        setDropping(false);
        addRef.current(event.payload.paths);
      }
    });
    return () => {
      cancelled = true;
      setDropping(false);
      un.then((f) => f()).catch(() => {});
    };
  }, [tabActive]);

  const busy = importing.length > 0;
  const empty = !loading && uploads.length === 0 && !busy;

  return (
    <>
      <div className="relative h-full">
        <div className="page-scroll">
          <div className="mx-auto max-w-5xl px-6 py-5">
            <header className="mb-4 flex items-start justify-between gap-4">
              <div className="min-w-0">
                <h2 className="text-[13px] font-medium text-foreground">Your files</h2>
                <p className="mt-0.5 text-[12px] text-muted-foreground">
                  Anything relevant that isn't on Canvas. PDFs and Office
                  documents are read and indexed like the rest of the subject —
                  other formats are kept, but not searchable.
                </p>
              </div>
              <Button size="sm" onClick={choose} disabled={busy}>
                {busy ? (
                  <CircleNotch className="animate-spin" aria-hidden />
                ) : (
                  <UploadSimple aria-hidden />
                )}
                {busy ? "Adding…" : "Add files"}
              </Button>
            </header>

            {problems.length > 0 && (
              <Alert variant="warning" className="mb-3 w-auto px-2.5 py-2">
                <Warning aria-hidden />
                <AlertDescription className="text-[11px] leading-snug">
                  {problems.map((p) => (
                    <span key={p} className="block">
                      {p}
                    </span>
                  ))}
                </AlertDescription>
              </Alert>
            )}

            {loading && uploads.length === 0 && !busy ? (
              <div className="space-y-2">
                {Array.from({ length: 4 }).map((_, i) => (
                  <Skeleton key={i} className="h-8 w-full" />
                ))}
              </div>
            ) : empty ? (
              <EmptyState onChoose={choose} />
            ) : (
              <div className="divide-y divide-border-subtle overflow-hidden rounded-lg border border-border">
                {importing.map((name) => (
                  <ImportingRow key={`importing:${name}`} name={name} />
                ))}
                {uploads.map((f) => (
                  <UploadRow
                    key={f.id}
                    file={f}
                    status={liveStatuses[f.relative_path]}
                    onDelete={() => setPendingDelete(f)}
                  />
                ))}
              </div>
            )}
          </div>
        </div>

        {/* The drop affordance is an overlay rather than a bordered zone in the
            layout: the whole page accepts files, and a zone that only covers
            part of it would be a lie about where a drop lands. */}
        <div
          aria-hidden
          className={cn(
            "pointer-events-none absolute inset-3 flex items-center justify-center rounded-xl border-2 border-dashed border-brand bg-brand/5 transition-opacity duration-150",
            dropping ? "opacity-100" : "opacity-0",
          )}
        >
          <span className="rounded-full bg-card px-3 py-1.5 text-[13px] font-medium text-brand shadow-sm">
            Drop to add to {subject.code}
          </span>
        </div>
      </div>

      <Dialog
        open={!!pendingDelete}
        onOpenChange={(open) => !open && !deleting && setPendingDelete(null)}
      >
        <DialogContent className="sm:max-w-sm" showCloseButton={false}>
          <DialogHeader>
            <DialogTitle>Remove this file?</DialogTitle>
            <DialogDescription>
              {pendingDelete
                ? `“${pendingDelete.filename}” is deleted from your library, along with what was read out of it. Nothing on Canvas changes, and your own copy of the file is untouched.`
                : ""}
            </DialogDescription>
          </DialogHeader>
          <DialogFooter>
            <Button
              variant="outline"
              disabled={deleting}
              onClick={() => setPendingDelete(null)}
            >
              Keep it
            </Button>
            <Button
              variant="destructive"
              disabled={deleting}
              onClick={async () => {
                if (!pendingDelete) return;
                setDeleting(true);
                try {
                  await removeUpload(pendingDelete);
                  setPendingDelete(null);
                } catch (e) {
                  setProblems([String(e)]);
                  setPendingDelete(null);
                } finally {
                  setDeleting(false);
                }
              }}
            >
              {deleting && <CircleNotch className="animate-spin" aria-hidden />}
              {deleting ? "Removing…" : "Remove"}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </>
  );
}

/** Nothing here yet — the one screen where the drop target is worth drawing in
 *  the layout, since there is no list for the overlay to sit over. */
function EmptyState({ onChoose }: { onChoose: () => void }) {
  return (
    <div className="flex flex-col items-center justify-center gap-3 rounded-xl border border-dashed border-border px-6 py-14 text-center">
      <UploadSimple size={24} className="text-muted-foreground/40" aria-hidden />
      <div className="space-y-1">
        <p className="text-sm text-foreground">Drop files here</p>
        <p className="text-[12px] text-muted-foreground">
          PDF, Word, PowerPoint and Excel are parsed and become searchable.
        </p>
      </div>
      <Button variant="outline" size="sm" onClick={onChoose}>
        Choose files
      </Button>
    </div>
  );
}

/** A file mid-copy. Same geometry as a real row so the list doesn't jump when
 *  the two swap. */
function ImportingRow({ name }: { name: string }) {
  return (
    <div className="flex items-center gap-3 px-3 py-2">
      <CircleNotch size={14} className="shrink-0 animate-spin opacity-60" aria-hidden />
      <span className="min-w-0 flex-1 truncate text-[12px] text-muted-foreground">
        {name}
      </span>
      <span className="shrink-0 text-[11px] text-muted-foreground">Adding…</span>
    </div>
  );
}

function UploadRow({
  file,
  status,
  onDelete,
}: {
  file: DbFile;
  status: string | undefined;
  onDelete: () => void;
}) {
  const Icon = fileIconFor(file.filename);
  // This session's live status if there is one, the column it was left at
  // otherwise. Only PDF-backed files have a state worth a word — for the rest
  // the column stays empty rather than saying "not searchable" on every row.
  const label = useMemo(() => {
    if (!isPdfBacked(file.filename)) return "";
    switch (status ?? file.parse_status) {
      case "error": return "failed";
      case "queued":
      case "running": return "reading";
      case "fast":
      case "quality": return "parsed";
      default: return "";
    }
  }, [file.filename, file.parse_status, status]);

  return (
    <div className="group flex items-center gap-3 px-3 py-2 transition-colors hover:bg-surface">
      <button
        onClick={() => openFileSmart(file)}
        className="flex min-w-0 flex-1 items-center gap-3 text-left"
      >
        <Icon size={14} className="shrink-0 opacity-60" aria-hidden />
        <span className="flex-1 truncate text-[12px] text-foreground">
          {file.filename}
        </span>
        {/* Fixed-width, right-aligned columns so every row lines up. */}
        <span
          className={cn(
            "w-14 shrink-0 text-right text-[10px] uppercase tracking-wide",
            status === "error"
              ? "text-destructive"
              : label === "reading"
                ? "text-muted-foreground"
                : "text-success",
          )}
        >
          {label}
        </span>
        <span className="w-17 shrink-0 text-right text-[11px] text-muted-foreground tabular-nums">
          {fmtSize(file.size_bytes)}
        </span>
      </button>

      <span className="flex w-3 shrink-0 items-center justify-center">
        <button
          onClick={onDelete}
          aria-label={`Remove ${file.filename}`}
          title="Remove from this subject"
          className="text-muted-foreground opacity-0 transition-[color,opacity] group-hover:opacity-100 hover:text-destructive focus-visible:opacity-100"
        >
          <Trash size={12} aria-hidden />
        </button>
      </span>
    </div>
  );
}
