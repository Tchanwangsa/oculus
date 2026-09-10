import { useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  ChatCircle,
  ChatsCircle,
  CheckCircle,
  Circle,
  Megaphone,
} from "@phosphor-icons/react";
import { Skeleton } from "@/components/ui/skeleton";
import { useSubjectFiles } from "@/hooks/useSubjectFiles";
import { useSubject } from "@/layouts/SubjectLayout";
import { openFileSmart } from "@/lib/openFile";
import { FileRecency } from "@/components/files/FileRecency";
import { humanizeSlug } from "@/lib/format";
import type { DbFile } from "@/lib/db";

/** Header metadata parsed from a scraped thread doc (see ed.rs). */
interface ThreadMeta {
  /** From the `**By:** name · 2026-08-07 15:42` line. */
  posted: Date | null;
  /** Ed's three thread types; resolved only applies to questions. */
  kind: "question" | "post" | "announcement" | null;
  /** true/false for questions (resolved/unresolved), null otherwise. */
  resolved: boolean | null;
  /** Board category path, e.g. "Assignments / A1". */
  category: string | null;
}

/**
 * The doc's header is `# title`, a `**#31 · question · Category · resolved**`
 * meta line, then the By-line — all written by ed.rs, so only the first few
 * hundred bytes matter.
 */
function parseThreadMeta(md: string): ThreadMeta {
  const head = md.slice(0, 500);

  const by = /^\*\*By:\*\* .* · (\d{4}-\d{2}-\d{2}) (\d{2}:\d{2})\s*$/m.exec(head);
  // Ed timestamps are written in the course's local timezone.
  const posted = by ? new Date(`${by[1]}T${by[2]}:00`) : null;

  const metaLine = /^\*\*(#\d+ ·[^*]*)\*\*\s*$/m.exec(head)?.[1] ?? "";
  const tokens = metaLine.split(" · ").map((t) => t.trim());
  const kind =
    (["question", "post", "announcement"] as const).find((k) => tokens.includes(k)) ?? null;
  const resolved = tokens.includes("resolved")
    ? true
    : tokens.includes("unresolved")
      ? false
      : null;
  // Whatever the meta line carries beyond number, type and status is the
  // board category path ("Assignments / A1").
  const category =
    tokens.find(
      (t) =>
        !t.startsWith("#") &&
        !["question", "post", "announcement", "resolved", "unresolved"].includes(t),
    ) ?? null;

  return {
    posted: posted && !Number.isNaN(posted.getTime()) ? posted : null,
    kind,
    resolved,
    category,
  };
}

/** file.id -> parsed header meta; empty until the docs load. */
function useThreadMetas(files: DbFile[]): Record<number, ThreadMeta> {
  const [metas, setMetas] = useState<Record<number, ThreadMeta>>({});

  useEffect(() => {
    if (files.length === 0) return;
    let cancelled = false;
    Promise.all(
      files.map(async (f) => {
        const md = await invoke<string>("read_course_file", {
          relativePath: f.relative_path,
        }).catch(() => "");
        return [f.id, parseThreadMeta(md)] as const;
      }),
    ).then((entries) => {
      if (!cancelled) setMetas(Object.fromEntries(entries));
    });
    return () => {
      cancelled = true;
    };
  }, [files]);

  return metas;
}

function fmtDay(d: Date): string {
  return d.toLocaleDateString("en-AU", { day: "numeric", month: "short" });
}

/**
 * The subject's Ed Discussion board, as scraped to `ed/NNNN-slug.md` — one
 * file per thread, replies included. Rows open in the peek.
 */
export default function SubjectDiscussionPage() {
  const subject = useSubject();
  const { byCategory, loading } = useSubjectFiles(subject.id);

  const threads = useMemo(
    () =>
      // Filenames are number-prefixed with Ed's per-course thread number, so
      // descending name order is newest first.
      [...byCategory.ed].sort((a, b) => b.filename.localeCompare(a.filename)),
    [byCategory.ed],
  );
  const metas = useThreadMetas(threads);

  if (loading && threads.length === 0) {
    return (
      <div className="page-scroll">
        <div className="mx-auto max-w-5xl px-6 py-6 space-y-2">
          {Array.from({ length: 5 }).map((_, i) => (
            <Skeleton key={i} className="h-8 w-full" />
          ))}
        </div>
      </div>
    );
  }

  if (threads.length === 0) {
    return (
      <div className="h-full flex flex-col items-center justify-center gap-2">
        <ChatsCircle size={24} className="text-muted-foreground/40" />
        <p className="text-sm text-muted-foreground">No Ed threads scraped.</p>
        <p className="text-xs text-muted-foreground/70 max-w-sm text-center">
          Run a sync — Ed connects automatically through your Canvas session
          when the subject has an Ed Discussion board.
        </p>
      </div>
    );
  }

  return (
    <div className="page-scroll">
      <div className="mx-auto max-w-5xl px-6 py-5">
        <div className="rounded-lg border border-border divide-y divide-border-subtle overflow-hidden">
          {threads.map((f) => {
            const number = /^(\d+)-/.exec(f.filename)?.[1];
            const meta = metas[f.id];
            return (
              <button
                key={f.id}
                onClick={() => openFileSmart(f)}
                className="w-full flex items-center gap-3 px-3 py-2.5 text-left hover:bg-surface transition-colors"
              >
                {/* One icon per row so titles align: questions show resolved
                    state, the other two types show what they are. */}
                <span className="shrink-0 w-3 flex items-center justify-center">
                  {meta?.resolved === true ? (
                    <CheckCircle size={11} weight="fill" className="text-success" />
                  ) : meta?.resolved === false ? (
                    <Circle size={11} className="text-warning" />
                  ) : meta?.kind === "announcement" ? (
                    <Megaphone size={11} className="text-muted-foreground/60" />
                  ) : meta?.kind === "question" ? (
                    // Doc predates the status token — unknown until a re-sync.
                    <Circle size={11} className="text-muted-foreground/40" />
                  ) : (
                    <ChatCircle size={11} className="text-muted-foreground/60" />
                  )}
                </span>
                <span className="text-[12px] text-foreground truncate flex-1">
                  {humanizeSlug(f.filename)}
                </span>
                {meta?.category && (
                  <span className="shrink-0 max-w-40 truncate text-[10px] text-muted-foreground">
                    {meta.category}
                  </span>
                )}
                <span className="shrink-0 w-12 text-right text-[11px] text-muted-foreground">
                  {meta?.posted ? fmtDay(meta.posted) : ""}
                </span>
                <span className="shrink-0 w-10 text-right text-[11px] text-muted-foreground tabular-nums">
                  {number ? `#${Number(number)}` : ""}
                </span>
                <span className="shrink-0 w-13 flex items-center justify-end">
                  <FileRecency file={f} />
                </span>
              </button>
            );
          })}
        </div>
      </div>
    </div>
  );
}
