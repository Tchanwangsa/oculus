import { useCallback, useEffect, useRef, useState } from "react";
import { useNavigate } from "react-router-dom";
import { Chat, FileText, FileVideo, Play, type Icon } from "@phosphor-icons/react";
import { SubjectIcon } from "@/components/subjects/SubjectIcon";
import { useScrollFade } from "@/hooks/useScrollFade";
import { lectureLabel } from "@/lib/calendar";
import { isVideoFile } from "@/lib/fileTypes";
import { displayCode, fmtAgo, sqliteUtcToMs } from "@/lib/format";
import { chatHref } from "@/lib/harness";
import { loadRecent, type RecentItem } from "@/lib/home";
import { LECTURES_CHANGED_EVENT, lecturePagePath } from "@/lib/lectures";
import {
  FILE_ACCESSED_EVENT,
  filePageHref,
  fileTitle,
  openFileSmart,
} from "@/lib/openFile";
import { openBeside } from "@/lib/tabRouters";
import { useHarnessStore } from "@/stores/harnessStore";
import { useHomeSection } from "./useHomeSection";

/**
 * Re-read on file access and lecture write-back. Not `LECTURE_PROGRESS_EVENT`:
 * it fires every few seconds of playback. Module-level for a stable reference.
 */
const EVENTS = [FILE_ACCESSED_EVENT, LECTURES_CHANGED_EVENT];

const MAX_CARDS = 12;

/**
 * The lectures, files and threads you were last in, newest first, as one
 * sideways-scrolling row of cards; absent when empty. Each card opens its item
 * the way its own list does.
 */
export function RecentRow() {
  const [items, setItems] = useState<RecentItem[]>([]);
  const navigate = useNavigate();
  // Threads carry only a subject id.
  const subjects = useHarnessStore((s) => s.subjects);
  const scrollRef = useRef<HTMLDivElement>(null);
  useScrollFade(scrollRef, "x", items.length > 0);

  useEffect(() => {
    void useHarnessStore.getState().loadSubjects();
  }, []);

  const reload = useCallback(() => {
    loadRecent(MAX_CARDS)
      .then(setItems)
      .catch((e) => {
        console.error(e);
        setItems([]);
      });
  }, []);

  useHomeSection(reload, EVENTS);

  if (items.length === 0) return null;

  return (
    <section>
      <h2 className="mb-2 px-0.5 text-[13px] font-semibold text-foreground">Recent</h2>
      {/* The bottom padding sits inside the scroller, between the cards and its bar. */}
      <div ref={scrollRef} className="overflow-x-auto">
        <div className="flex w-max gap-3 pb-2">
          {items.map((item) => (
            <Card
              key={cardKey(item)}
              item={item}
              threadSubject={threadSubjectCode(item, subjects)}
              onOpenThread={(id, title) => navigate(chatHref(id, title))}
            />
          ))}
        </div>
      </div>
    </section>
  );
}

/** Ids collide across kinds, so the kind is part of the key. */
function cardKey(item: RecentItem): string {
  if (item.kind === "lecture") return `lecture:${item.lecture.id}`;
  if (item.kind === "file") return `file:${item.file.id}`;
  return `thread:${item.thread.id}`;
}

function threadSubjectCode(
  item: RecentItem,
  subjects: { id: number; code: string }[],
): string | null {
  if (item.kind !== "thread" || item.thread.subject_id == null) return null;
  return subjects.find((s) => s.id === item.thread.subject_id)?.code ?? null;
}

function Card({
  item,
  threadSubject,
  onOpenThread,
}: {
  item: RecentItem;
  /** A thread's raw subject code, looked up by the row. */
  threadSubject: string | null;
  onOpenThread: (id: number, title: string | null) => void;
}) {
  const ago = fmtAgo(sqliteUtcToMs(item.at));

  let Glyph: Icon = Chat;
  let title = "";
  let code: string | null = null;
  /** Watched fraction, lectures only. */
  let watched: number | null = null;
  let open = () => {};
  /* ⌘-click target (`lib/newTabClicks.ts`); threads have no page of their own. */
  let tabHref: string | null = null;

  if (item.kind === "lecture") {
    const { lecture } = item;
    Glyph = Play;
    // `lectureLabel` drops the subject code the meta line already shows.
    title = lectureLabel(lecture.title, lecture.subject_code);
    code = lecture.subject_code;
    watched = lecture.duration_seconds > 0
      ? Math.min(1, lecture.progress_seconds / lecture.duration_seconds)
      : 0;
    open = () => openBeside(lecturePagePath(lecture));
    tabHref = lecturePagePath(lecture);
  } else if (item.kind === "file") {
    const { file } = item;
    Glyph = isVideoFile(file.filename) ? FileVideo : FileText;
    title = fileTitle(file);
    code = file.subject_code;
    // `openFileSmart` records the access and fires the event itself.
    open = () => openFileSmart(file);
    tabHref = filePageHref(file);
  } else {
    const { thread } = item;
    title = thread.title ?? "";
    code = threadSubject;
    open = () => onOpenThread(thread.id, thread.title);
  }

  return (
    <button
      type="button"
      className="relative flex w-[200px] shrink-0 flex-col gap-2 overflow-hidden rounded-lg border border-border p-3 text-left transition-colors hover:bg-surface"
      data-tab-href={tabHref ?? undefined}
      onClick={open}
    >
      <Glyph size={14} className="shrink-0 text-muted-foreground" />
      <span className="truncate text-[12px] leading-snug text-foreground">{title}</span>
      <span className="mt-auto flex min-w-0 items-center gap-1.5 text-[11px] text-muted-foreground">
        {code && <SubjectIcon code={code} size={12} />}
        {/* No subject on a thread means library-wide, which is worth naming. */}
        <span className="truncate">{code ? displayCode(code) : "Library"}</span>
        <span className="shrink-0">·</span>
        <span className="shrink-0 tabular-nums">{ago}</span>
      </span>
      {watched != null && (
        // Flush along the bottom edge, so it costs the layout nothing.
        <span className="absolute inset-x-0 bottom-0 h-[3px] bg-muted">
          <span className="block h-full bg-brand" style={{ width: `${watched * 100}%` }} />
        </span>
      )}
    </button>
  );
}
