import { useMemo } from "react";
import {
  ChatCircle,
  ChatsCircle,
  CheckCircle,
  Circle,
  Megaphone,
} from "@phosphor-icons/react";
import { SubjectLoading, SubjectPage, SubjectEmpty } from "@/components/subjects/SubjectPage";
import { useSubjectFiles } from "@/hooks/data/useSubjectFiles";
import { useSubject } from "@/layouts/SubjectLayout";
import { filePageHref, openFileSmart } from "@/lib/files/openFile";
import { FileRecency } from "@/components/files/FileRecency";
import { fmtShortDate, humanizeSlug } from "@/lib/format/format";
import type { DbFile } from "@/lib/db";
import { useCourseFileData } from "@/hooks/data/useCourseFileData";
import { ListCard } from "@/components/ui/layout/PageParts";

/** Header metadata parsed from a scraped thread doc (see sources/ed/). */
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

/** The header sources/ed/ writes: `# title`, a `**#31 · question · Category ·
 *  resolved**` meta line, then the By-line. */
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
  // The rest of the meta line is the category path.
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

function parseThreadEntry(md: string, file: DbFile) {
  return [file.id, parseThreadMeta(md)] as const;
}

/**
 * The subject's Ed Discussion board, as scraped to `ed/NNNN-slug.md` — one
 * file per thread, replies included. Rows open in the side panel.
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
  const entries = useCourseFileData(threads, loading, parseThreadEntry);
  const metas = useMemo(() => Object.fromEntries(entries ?? []), [entries]);

  if (loading && threads.length === 0) {
    return <SubjectLoading count={5} />;
  }

  if (threads.length === 0) {
    return (
      <SubjectEmpty icon={<ChatsCircle size={24} className="text-muted-foreground/40" />} title="No Ed threads scraped.">
        <p className="text-xs text-muted-foreground/70 max-w-sm text-center">
          Run a sync — Ed connects automatically through your Canvas session
          when the subject has an Ed Discussion board.
        </p>
      </SubjectEmpty>
    );
  }

  return (
    <SubjectPage>
      <ListCard>
        {threads.map((f) => {
          const number = /^(\d+)-/.exec(f.filename)?.[1];
          const meta = metas[f.id];
          return (
            <button
              key={f.id}
              data-tab-href={filePageHref(f) ?? undefined}
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
                {meta?.posted ? fmtShortDate(meta.posted) : ""}
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
      </ListCard>
    </SubjectPage>
  );
}
