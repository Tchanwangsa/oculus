import { useCallback, useState, type ReactNode } from "react";
import { useNavigate } from "react-router-dom";
import { Chat, FileText, Play } from "@phosphor-icons/react";
import { lectureLabel } from "@/lib/calendar";
import { displayCode, fmtAgo, sqliteUtcToMs } from "@/lib/format";
import { loadContinue, type ContinueItem } from "@/lib/home";
import {
  LECTURES_CHANGED_EVENT,
  lecturePagePath,
  progressLabel,
} from "@/lib/lectures";
import {
  FILE_ACCESSED_EVENT,
  filePageHref,
  fileTitle,
  openFileSmart,
} from "@/lib/openFile";
import { useHarnessStore } from "@/stores/harnessStore";
import { chatHref } from "@/lib/harness";
import { useSidePanelStore } from "@/stores/sidePanelStore";
import { ROW, Section } from "./Section";
import { useHomeSection } from "./useHomeSection";

/**
 * Re-read on file access and lecture write-back. Not `LECTURE_PROGRESS_EVENT`:
 * it fires every few seconds of playback. Module-level for a stable reference.
 */
const EVENTS = [FILE_ACCESSED_EVENT, LECTURES_CHANGED_EVENT];

const MAX_ROWS = 4;

/**
 * The lecture, file or thread you were last in, newest first; absent when
 * empty. Each row opens its item the way its own list does.
 */
export function ContinueSection() {
  const [items, setItems] = useState<ContinueItem[]>([]);
  const navigate = useNavigate();
  // Threads carry only a subject id; `HomePage` loads the shared subject list.
  const subjects = useHarnessStore((s) => s.subjects);

  const reload = useCallback(() => {
    loadContinue(MAX_ROWS)
      .then(setItems)
      .catch((e) => {
        console.error(e);
        setItems([]);
      });
  }, []);

  useHomeSection(reload, EVENTS);

  if (items.length === 0) return null;

  return (
    <Section title="Continue">
      {items.map((item) => (
        <Row
          key={rowKey(item)}
          item={item}
          subjectCode={threadSubjectCode(item, subjects)}
          onOpenThread={(id) => navigate(chatHref(id, item.kind === "thread" ? item.thread.title : null))}
        />
      ))}
    </Section>
  );
}

/** Ids collide across kinds, so the kind is part of the key. */
function rowKey(item: ContinueItem): string {
  if (item.kind === "lecture") return `lecture:${item.lecture.id}`;
  if (item.kind === "file") return `file:${item.file.id}`;
  return `thread:${item.thread.id}`;
}

function threadSubjectCode(
  item: ContinueItem,
  subjects: { id: number; code: string }[],
): string | null {
  if (item.kind !== "thread" || item.thread.subject_id == null) return null;
  const s = subjects.find((s) => s.id === item.thread.subject_id);
  return s ? displayCode(s.code) : null;
}

function Row({
  item,
  subjectCode,
  onOpenThread,
}: {
  item: ContinueItem;
  subjectCode: string | null;
  onOpenThread: (id: number) => void;
}) {
  const ago = fmtAgo(sqliteUtcToMs(item.at));

  let Glyph = Chat;
  let title = "";
  let sub: ReactNode = null;
  let open = () => {};
  /* ⌘-click target (`lib/newTabClicks.ts`); threads have no page of their own. */
  let tabHref: string | null = null;

  if (item.kind === "lecture") {
    const { lecture } = item;
    // `lectureLabel` drops the subject code the sub-line already shows.
    const progress = progressLabel(lecture);
    Glyph = Play;
    title = lectureLabel(lecture.title, lecture.subject_code);
    sub = (
      <>
        {displayCode(lecture.subject_code)}
        {" · "}
        {/* Only the progress fragment is coloured. */}
        <span className={progress.color}>{progress.text}</span>
      </>
    );
    open = () => useSidePanelStore.getState().open({ kind: "lecture", lecture });
    tabHref = lecturePagePath(lecture);
  } else if (item.kind === "file") {
    const { file } = item;
    Glyph = FileText;
    title = fileTitle(file);
    sub = displayCode(file.subject_code);
    // `openFileSmart` records the access and fires the event itself.
    open = () => openFileSmart(file);
    tabHref = filePageHref(file);
  } else {
    const { thread } = item;
    Glyph = Chat;
    title = thread.title ?? "";
    // No subject means library-wide, which is worth naming.
    sub = subjectCode ?? "Library";
    open = () => onOpenThread(thread.id);
  }

  return (
    <button
      type="button"
      className={ROW}
      data-tab-href={tabHref ?? undefined}
      onClick={open}
    >
      <Glyph size={13} className="shrink-0 text-muted-foreground" />
      <span className="min-w-0 flex-1">
        <span className="block truncate text-[12px] text-foreground">{title}</span>
        <span className="block truncate text-[11px] text-muted-foreground">{sub}</span>
      </span>
      <span className="shrink-0 text-[11px] tabular-nums text-muted-foreground">{ago}</span>
    </button>
  );
}
