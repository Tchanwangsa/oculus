import { useEffect, useMemo, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import {
  ArrowsClockwise,
  BookOpen,
  CalendarBlank,
  Chat,
  GearSix,
  MagnifyingGlass,
  VideoCamera,
  type Icon as PhosphorIcon,
} from "@phosphor-icons/react";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogTitle,
} from "@/components/ui/dialog";
import { navigateActive } from "@/lib/tabRouters";
import { usePaletteStore } from "@/stores/paletteStore";
import { useTabStore } from "@/stores/tabStore";
import { useSubjects } from "@/hooks/useSubjects";
import { filePagePath } from "@/components/panel/FilePanel";
import { SubjectIcon } from "@/components/subjects/SubjectIcon";
import { categoryIconFor, isPdfBacked } from "@/lib/fileTypes";
import { fileTitle, openFileSmart } from "@/lib/openFile";
import { fmtLectureDate, lecturePagePath } from "@/lib/lectures";
import { displayCode, displayName } from "@/lib/format";
import {
  searchLibraryFiles,
  searchLibraryLectures,
  type LibraryFileHit,
  type LibraryLectureHit,
  type Subject,
} from "@/lib/db";
import { cn } from "@/lib/utils";

const FILE_LIMIT = 8;
const LECTURE_LIMIT = 4;
const SUBJECT_LIMIT = 5;
/** With nothing typed the list is a landing, not a dump — five of each. */
const IDLE_LIMIT = 5;

/** The places the palette can send you that aren't a document. */
const PLACES: { label: string; path: string; icon: PhosphorIcon }[] = [
  { label: "Chat", path: "/chat", icon: Chat },
  { label: "Calendar", path: "/calendar", icon: CalendarBlank },
  { label: "Subjects", path: "/subjects", icon: BookOpen },
  { label: "Sync", path: "/sync", icon: ArrowsClockwise },
  { label: "Settings · Canvas", path: "/settings/canvas", icon: GearSix },
  { label: "Settings · AI", path: "/settings/ai", icon: GearSix },
  { label: "Settings · Storage", path: "/settings/storage", icon: GearSix },
  { label: "Settings · Library", path: "/settings/library", icon: GearSix },
];

/** One row. Everything the palette offers is either a route or an escape
 *  hatch that runs itself — nothing needs both. */
interface Item {
  key: string;
  icon: React.ReactNode;
  label: string;
  /** Right-aligned: the subject a document belongs to, a subject's term. */
  meta?: string;
  path?: string;
  run?: () => void;
}

interface Section {
  heading: string;
  items: Item[];
}

/**
 * Every typed word must appear somewhere, in any order — the same rule the SQL
 * side applies to files and lectures, so what the in-memory lists (subjects,
 * places) do and what the database does stay one behaviour.
 */
function matchesAll(haystack: string, query: string): boolean {
  const hay = haystack.toLowerCase();
  return query
    .toLowerCase()
    .split(/\s+/)
    .filter(Boolean)
    .every((w) => hay.includes(w));
}

/**
 * The ⌘K palette: one field over everything you can reach — subjects, files,
 * lectures and the app's own pages.
 *
 * It matches **titles**, from SQLite, on every keystroke. Not the semantic page
 * index: that is an embedding round-trip through the sidecar (see
 * `docs/retrieval.md`) and belongs to a question you ask Chat, not to a field
 * you are still typing in.
 *
 * Enter goes there in the current tab and ⌘↵ in a new one — the rule the
 * sidebar's Recent rows already follow.
 */
export default function CommandPalette() {
  const open = usePaletteStore((s) => s.open);
  const setOpen = usePaletteStore((s) => s.setOpen);
  const toggle = usePaletteStore((s) => s.toggle);

  // ⌘K is a menu item, not a key handler: macOS gives the menu bar every
  // ⌘-key before a webview sees it. That is also what makes the palette reach
  // you while a browser tab's native page holds focus — which is exactly when
  // you most want a way out of it. See `app/src-tauri/src/menu.rs`.
  useEffect(() => {
    const pending = listen("menu-search", () => toggle());
    return () => {
      pending.then((un) => un()).catch(() => {});
    };
  }, [toggle]);

  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogContent
        showCloseButton={false}
        // Palettes hang from the top of the window rather than sitting in the
        // middle of it: the list grows downwards and the field must not move
        // as it does.
        className="top-[14%] translate-y-0 gap-0 overflow-hidden rounded-xl border-border bg-popover p-0 shadow-lg sm:max-w-xl"
      >
        <DialogTitle className="sr-only">Search</DialogTitle>
        <DialogDescription className="sr-only">
          Find a subject, file, lecture or page.
        </DialogDescription>
        {/* Mounted with the dialog, so each open starts on an empty field and
            a fresh read of the library. */}
        <PaletteBody onClose={() => setOpen(false)} />
      </DialogContent>
    </Dialog>
  );
}

function PaletteBody({ onClose }: { onClose: () => void }) {
  const addTab = useTabStore((s) => s.addTab);
  const { subjects, current } = useSubjects();

  const [query, setQuery] = useState("");
  const [files, setFiles] = useState<LibraryFileHit[]>([]);
  const [lectures, setLectures] = useState<LibraryLectureHit[]>([]);
  const [index, setIndex] = useState(0);
  const listRef = useRef<HTMLDivElement>(null);
  // The query the lists belong to, so a slow read landing after the field
  // moved on cannot overwrite a newer one.
  const latest = useRef("");

  useEffect(() => {
    const token = query;
    latest.current = token;
    const idle = query.trim() === "";
    Promise.all([
      searchLibraryFiles(query, idle ? IDLE_LIMIT : FILE_LIMIT),
      // With nothing typed there is no such thing as a relevant lecture, only
      // a recent one — and the recent files above already say that better.
      idle
        ? Promise.resolve<LibraryLectureHit[]>([])
        : searchLibraryLectures(query, LECTURE_LIMIT),
    ])
      .then(([f, l]) => {
        if (latest.current !== token) return;
        setFiles(f);
        setLectures(l);
      })
      .catch(() => {});
  }, [query]);

  const sections = useMemo<Section[]>(() => {
    const idle = query.trim() === "";

    const fileItems: Item[] = files.map((f) => {
      const Icon = categoryIconFor(f);
      return {
        key: `file:${f.id}`,
        icon: <Icon size={15} className="shrink-0 text-muted-foreground" />,
        label: fileTitle(f),
        meta: displayCode(f.subject_code),
        // A binary we cannot render — a .zip, a .xlsx — has no page to go to,
        // so it leaves for the system viewer the way any list row does.
        ...(f.category === "file" && !isPdfBacked(f.filename)
          ? { run: () => openFileSmart(f) }
          : { path: filePagePath(f.subject_id, f.relative_path) }),
      };
    });

    const subjectItems: Item[] = (idle ? current : subjects)
      .filter((s) => idle || matchesAll(subjectHaystack(s), query))
      .slice(0, idle ? IDLE_LIMIT : SUBJECT_LIMIT)
      .map((s) => ({
        key: `subject:${s.id}`,
        icon: <SubjectIcon code={s.code} size={15} />,
        label: displayName(s.name, s.code),
        meta: displayCode(s.code),
        path: `/subjects/${s.id}`,
      }));

    // Echo360 titles a capture by its room and timetable slot, so a subject's
    // lectures all read alike — the date is the half that tells them apart,
    // which is why it is here and not just the subject code.
    const lectureItems: Item[] = lectures.map((l) => ({
      key: `lecture:${l.id}`,
      icon: <VideoCamera size={15} className="shrink-0 text-muted-foreground" />,
      label: l.title,
      meta: `${displayCode(l.subject_code)} · ${fmtLectureDate(l.date)}`,
      path: lecturePagePath(l),
    }));

    const placeItems: Item[] = PLACES.filter(
      (p) => idle || matchesAll(p.label, query),
    ).map((p) => ({
      key: `place:${p.path}`,
      icon: <p.icon size={15} className="shrink-0 text-muted-foreground" />,
      label: p.label,
      path: p.path,
    }));

    // Two orders, because the useful first row differs. Idle, it is the file
    // you were last in; searching, it is whatever best matches what you typed,
    // and a subject beats a document that merely mentions its code.
    const order: Section[] = idle
      ? [
          { heading: "Recent", items: fileItems },
          { heading: "Subjects", items: subjectItems },
          { heading: "Go to", items: placeItems },
        ]
      : [
          { heading: "Subjects", items: subjectItems },
          { heading: "Files", items: fileItems },
          { heading: "Lectures", items: lectureItems },
          { heading: "Go to", items: placeItems },
        ];
    return order.filter((s) => s.items.length > 0);
  }, [query, files, lectures, subjects, current]);

  const flat = useMemo(() => sections.flatMap((s) => s.items), [sections]);
  /** Where each section starts in `flat`, so a row knows its own keyboard
   *  index without the render walking a counter. */
  const offsets = useMemo(() => {
    let n = 0;
    return sections.map((s) => {
      const start = n;
      n += s.items.length;
      return start;
    });
  }, [sections]);
  // Results landing after a keystroke can shorten the list under the
  // highlight, so the live selection is always the clamped one.
  const selected = Math.min(index, Math.max(0, flat.length - 1));

  // A new list is a new first row; clamping instead would leave the highlight
  // on whatever happened to land at the old offset.
  useEffect(() => setIndex(0), [query]);

  // Follow the highlight with the scroll, addressed through the DOM rather
  // than a ref per row: the rows are rebuilt on every keystroke and a map of
  // refs would be a second thing to keep in step with `flat`.
  useEffect(() => {
    listRef.current
      ?.querySelector(`[data-index="${selected}"]`)
      ?.scrollIntoView({ block: "nearest" });
  }, [selected]);

  function go(item: Item, newTab: boolean) {
    onClose();
    if (item.run) {
      item.run();
      return;
    }
    if (!item.path) return;
    if (newTab) addTab(item.path);
    else navigateActive(item.path);
  }

  function onKeyDown(e: React.KeyboardEvent) {
    if (flat.length === 0) return;
    if (e.key === "ArrowDown") {
      e.preventDefault();
      setIndex((i) => (i + 1) % flat.length);
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      setIndex((i) => (i - 1 + flat.length) % flat.length);
    } else if (e.key === "Enter") {
      e.preventDefault();
      go(flat[selected], e.metaKey || e.ctrlKey);
    }
  }

  return (
    <div onKeyDown={onKeyDown}>
      <div className="flex items-center gap-2.5 border-b border-border-subtle px-4">
        <MagnifyingGlass size={15} className="shrink-0 text-muted-foreground" />
        <input
          autoFocus
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          placeholder="Search subjects, files, lectures…"
          className="h-11 flex-1 bg-transparent text-sm text-foreground outline-none placeholder:text-muted-foreground"
        />
      </div>

      <div ref={listRef} className="max-h-[min(58vh,380px)] overflow-y-auto py-1.5">
        {flat.length === 0 ? (
          <p className="px-4 py-6 text-center text-[12.5px] text-muted-foreground">
            No matches.
          </p>
        ) : (
          sections.map((section, si) => (
            <div key={section.heading} className="pb-1 last:pb-0">
              {/* No icon: section headers are labels, not rows. */}
              <div className="px-4 pb-0.5 pt-1.5 text-[11px] font-medium tracking-wide text-muted-foreground">
                {section.heading}
              </div>
              {section.items.map((item, ii) => {
                const i = offsets[si] + ii;
                return (
                  <button
                    key={item.key}
                    type="button"
                    data-index={i}
                    // `mousemove`, not `mouseenter`: arrowing through a list
                    // that scrolls under a resting pointer would otherwise
                    // hand the selection straight back to the mouse.
                    onMouseMove={() => setIndex(i)}
                    onClick={(e) => go(item, e.metaKey || e.ctrlKey)}
                    className={cn(
                      "mx-1.5 flex w-[calc(100%-0.75rem)] items-center gap-2.5 rounded-md px-2.5 py-1.5 text-left text-[12.5px]",
                      i === selected
                        ? "bg-accent text-foreground"
                        : "text-foreground/90",
                    )}
                  >
                    {item.icon}
                    <span className="min-w-0 flex-1 truncate">{item.label}</span>
                    {item.meta && (
                      <span className="shrink-0 text-[11px] text-muted-foreground">
                        {item.meta}
                      </span>
                    )}
                  </button>
                );
              })}
            </div>
          ))
        )}
      </div>

      <div className="flex items-center gap-3 border-t border-border-subtle px-4 py-2 text-[11px] text-muted-foreground">
        <Hint keys="↵" label="Open" />
        <Hint keys="⌘↵" label="Open in new tab" />
        <Hint keys="esc" label="Close" />
      </div>
    </div>
  );
}

/** Everything a subject answers to: its display name and its code, with and
 *  without the term suffix. */
function subjectHaystack(s: Subject): string {
  return `${displayName(s.name, s.code)} ${s.code} ${displayCode(s.code)}`;
}

function Hint({ keys, label }: { keys: string; label: string }) {
  return (
    <span className="flex items-center gap-1.5">
      <kbd className="rounded border border-border bg-surface px-1 py-0.5 text-[10px] leading-none text-muted-foreground">
        {keys}
      </kbd>
      {label}
    </span>
  );
}
