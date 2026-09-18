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
import { useSidePanelStore } from "@/stores/sidePanelStore";
import { ROW, Section } from "./Section";
import { useHomeSection } from "./useHomeSection";

/**
 * Opening something from Home re-ranks this very list, so both stamps have to
 * come back: `FILE_ACCESSED_EVENT` when a file's access is recorded, and
 * `LECTURES_CHANGED_EVENT` when a lecture's progress is written back.
 *
 * Deliberately **not** `LECTURE_PROGRESS_EVENT`: that one fires every five
 * seconds of playback, so subscribing here would have Home re-querying three
 * tables for the length of a lecture playing in another tab, to move a row
 * nobody is looking at. The write-back event is the one that matters, and the
 * front edge in `useHomeSection` catches whatever the playback missed.
 *
 * Module-level so the reference is stable — see `useHomeSection`.
 */
const EVENTS = [FILE_ACCESSED_EVENT, LECTURES_CHANGED_EVENT];

/** Four rows. This is a way back into one thing, not a history page — a fifth
 *  and sixth entry are far enough back that the search field is the faster
 *  route to them. */
const MAX_ROWS = 4;

/**
 * The lecture, file or conversation you were last in, newest first.
 *
 * Empty is the common state on a fresh library and on a Monday morning, and
 * the section simply is not there then — a box saying "nothing recent" is the
 * placeholder UI this app doesn't ship.
 *
 * Every row opens the thing the way its own list opens it: a lecture into the
 * side panel, a file through `openFileSmart` (PDF in the panel, anything we
 * can't render handed to the system viewer), a thread by opening it in the
 * harness store and going to Chat. Nothing here is a second, Home-only way
 * into any of them.
 */
export function ContinueSection() {
  const [items, setItems] = useState<ContinueItem[]>([]);
  const navigate = useNavigate();
  // Threads carry a subject id and no code. `HomePage` loads the subject list
  // into the shared store once for the whole page — the composer reads it too —
  // so reading the slice re-renders the rows when it lands rather than costing
  // a second query for the same table.
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
          onOpenThread={(id) => {
            void useHarnessStore.getState().open(id);
            navigate("/chat");
          }}
        />
      ))}
    </Section>
  );
}

/** Ids collide across kinds — a file 4 and a thread 4 are both `4` — so the
 *  kind is part of the key. */
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
  /* Where ⌘-click leads (`lib/newTabClicks.ts`). A lecture and a file both
     have a page of their own; a thread does not — it is selected in the one
     chat page rather than routed to — so its row has no new-tab form and is
     left without one. */
  let tabHref: string | null = null;

  if (item.kind === "lecture") {
    const { lecture } = item;
    // The subject code is already the front of an Echo360 title, and the row
    // says it again underneath — `lectureLabel` takes it off the front.
    const progress = progressLabel(lecture);
    Glyph = Play;
    title = lectureLabel(lecture.title, lecture.subject_code);
    sub = (
      <>
        {displayCode(lecture.subject_code)}
        {" · "}
        {/* The colour is the progress fragment's alone: "34:12 left" is the
            part that is warm, not the subject code in front of it. */}
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
    // `openFileSmart` records the access itself and fires the event this list
    // listens on; stamping it again here would be one write and one reload too
    // many.
    open = () => openFileSmart(file);
    tabHref = filePageHref(file);
  } else {
    const { thread } = item;
    Glyph = Chat;
    title = thread.title ?? "";
    // A thread with no subject is scoped to the whole library, which is a
    // place and so worth naming — the blank would read as missing data.
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
