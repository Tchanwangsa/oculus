import {
  Fragment,
  isValidElement,
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  type KeyboardEvent as ReactKeyboardEvent,
  type ReactNode,
} from "react";
import { syntaxTree } from "@codemirror/language";
import { EditorState } from "@codemirror/state";
import { EditorView, type ViewUpdate } from "@codemirror/view";

import { Alert, AlertDescription } from "@/components/ui/alert";
import { PillTabs } from "@/components/ui/PillTabs";
import { useTabActive } from "@/components/tabs/TabContext";
import { useDataDir } from "@/hooks/useDataDir";
import { useFileDrop } from "@/hooks/useFileDrop";
import { useNow } from "@/hooks/useNow";
import { useSubjects } from "@/hooks/useSubjects";
import { imagePaths } from "@/lib/attachments";
import {
  attachDocumentFile,
  attachDocumentImage,
  cancelDocumentSuggestion,
  pickDocumentImages,
  renameDocument,
  saveDocument,
  suggestDocument,
} from "@/lib/documents";
import {
  displayCode,
  displayName,
  fmtFullStamp,
  fmtRecent,
  sqliteUtcToMs,
} from "@/lib/format";
import { libraryImageSrc } from "@/lib/libraryLinks";
import { fileTitle } from "@/lib/openFile";
import { navigateActive } from "@/lib/tabRouters";
import { cn } from "@/lib/utils";
import type { DbFile } from "@/lib/db";
import { readCourseFile } from "@/lib/courseFiles";
import { LoadingFill } from "@/components/ui/PageParts";

import {
  activeFormats,
  insertImage,
  NO_FORMATS,
  sameFormats,
  type ActiveFormats,
} from "./editor/commands";
import {
  suggestCompartment,
  suggestExtension,
  type SuggestConfig,
  type SuggestStatus,
} from "./editor/aiSuggest";
import { liveCompartment, modeExtension, noteExtensions } from "./editor/extensions";
import { hostCompartment, noteHost, openNoteLink, type NoteHost } from "./editor/host";
import { syncLiveFocus } from "./editor/livePreview";
import { Toolbar } from "./editor/Toolbar";
import { SuggestToggle } from "./SuggestToggle";

export type EditorMode = "live" | "raw";

/** The header's status word; `idle` is blank. */
export type SaveStatus =
  | { state: "idle" }
  | { state: "saving" }
  | { state: "saved" }
  | { state: "error"; message: string };

const MODES = [
  { value: "live", label: "Live" },
  { value: "raw", label: "Raw" },
] as const;

/** How long after the last keystroke the draft goes to disk. */
const SAVE_DELAY_MS = 600;

/** A freshly created note, still wearing the name Rust gave it. */
const UNTITLED = /^Untitled(?:-\d+)?$/;

/** The save word, the AI-suggestions toggle and the Live/Raw pills, drawn in
 *  the host page's header. */
export function DocumentControls({
  mode,
  onMode,
  status,
  suggestions,
  onSuggestions,
  suggestStatus,
}: {
  mode: EditorMode;
  onMode: (mode: EditorMode) => void;
  status: SaveStatus;
  suggestions: boolean;
  onSuggestions: (on: boolean) => void;
  suggestStatus: SuggestStatus;
}) {
  const word =
    status.state === "saving" ? "Saving…"
    : status.state === "saved" ? "Saved"
    : status.state === "error" ? status.message
    : "";
  return (
    <div className="flex shrink-0 items-center gap-3">
      <span
        className={cn(
          "max-w-64 truncate text-[11px]",
          status.state === "error" ? "text-destructive" : "text-muted-foreground",
        )}
        title={status.state === "error" ? status.message : undefined}
      >
        {word}
      </span>
      <SuggestToggle on={suggestions} onChange={onSuggestions} status={suggestStatus} />
      <PillTabs tabs={MODES} value={mode} onChange={onMode} />
    </div>
  );
}

/**
 * A markdown note, edited in place in CodeMirror (`./editor/`); controls live
 * in the host's header. The file's text is the document — nothing is
 * re-serialised. Live mode renders markdown around the caret, Raw is the same
 * editor without that.
 *
 * The draft lives in a ref so every save (timer, blur, ⌘S, unmount) takes the
 * latest text, and writes are serialised — one in flight, looping until the
 * draft on disk is current — so saves never land out of order.
 */
export function DocumentEditor({
  file,
  files,
  mode,
  onMode,
  onStatus,
  suggestions,
  onSuggestStatus,
}: {
  file: DbFile;
  /** The subject's files, for links to resolve against. */
  files: DbFile[];
  mode: EditorMode;
  onMode: (mode: EditorMode) => void;
  onStatus: (status: SaveStatus) => void;
  /** Inline AI suggestions (`./editor/aiSuggest.ts`) on or off. */
  suggestions: boolean;
  onSuggestStatus: (status: SuggestStatus) => void;
}) {
  const tabActive = useTabActive();
  /** The text as loaded; after that the editor holds it. */
  const [initial, setInitial] = useState<string | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [title, setTitle] = useState(() => fileTitle(file));
  /** Picture-attach failures; separate from the save word, which the next
   *  keystroke would overwrite. */
  const [attachError, setAttachError] = useState<string | null>(null);
  const [view, setView] = useState<EditorView | null>(null);
  const [active, setActive] = useState<ActiveFormats>(NO_FORMATS);
  /** The last save this mount made, and the words on disk — both move on
   *  save, not per keystroke. The row's `modified_at` isn't re-read on save. */
  const [savedAt, setSavedAt] = useState<number | null>(null);
  const [words, setWords] = useState(0);

  const draft = useRef("");
  const saved = useRef("");
  const timer = useRef<number | null>(null);
  const inFlight = useRef<Promise<void> | null>(null);
  const viewRef = useRef<EditorView | null>(null);
  const editorRef = useRef<HTMLDivElement>(null);
  const titleRef = useRef<HTMLInputElement>(null);
  const toolbarRef = useRef<HTMLDivElement>(null);
  /** The whole page is the drop target, not just the text. */
  const pageRef = useRef<HTMLDivElement>(null);

  // Updated after commit, not during render, so the cleanup on a file switch
  // (which React runs before new effects) still sees the old file.
  const fileRef = useRef(file);
  useEffect(() => {
    fileRef.current = file;
  });
  const statusRef = useRef(onStatus);
  statusRef.current = onStatus;
  const filesRef = useRef(files);
  filesRef.current = files;
  const modeRef = useRef(mode);
  modeRef.current = mode;
  const suggestionsRef = useRef(suggestions);
  suggestionsRef.current = suggestions;
  const suggestStatusRef = useRef(onSuggestStatus);
  suggestStatusRef.current = onSuggestStatus;

  /** Stable, so the toggle only swaps the compartment; the path is read per
   *  request, so a rename needs nothing. */
  const suggestConfig = useMemo<SuggestConfig>(
    () => ({
      fetch: ({ requestId, before, after }) =>
        suggestDocument({ requestId, path: fileRef.current.relative_path, before, after }),
      cancel: () => void cancelDocumentSuggestion().catch(console.error),
      onStatus: (s) => suggestStatusRef.current(s),
    }),
    [],
  );

  /** Write the draft if it differs from disk. Idempotent. */
  const flush = useCallback((): Promise<void> => {
    if (timer.current != null) {
      window.clearTimeout(timer.current);
      timer.current = null;
    }
    if (inFlight.current) return inFlight.current;
    if (draft.current === saved.current) return Promise.resolve();
    const run = (async () => {
      while (draft.current !== saved.current) {
        const content = draft.current;
        const id = fileRef.current.id;
        statusRef.current({ state: "saving" });
        try {
          await saveDocument(fileRef.current, content);
          saved.current = content;
          // A save that lands after a file switch belongs to the old note.
          if (fileRef.current.id === id) {
            setSavedAt(Date.now());
            setWords(countWords(content));
          }
          statusRef.current({ state: "saved" });
        } catch (e) {
          // Not retried: the next keystroke schedules another attempt.
          statusRef.current({ state: "error", message: String(e) });
          break;
        }
      }
      inFlight.current = null;
    })();
    inFlight.current = run;
    return run;
  }, []);

  const schedule = useCallback(() => {
    if (timer.current != null) window.clearTimeout(timer.current);
    timer.current = window.setTimeout(() => {
      timer.current = null;
      void flush();
    }, SAVE_DELAY_MS);
  }, [flush]);

  // Keyed on the row, not its path: a rename must not reload the draft.
  useEffect(() => {
    let live = true;
    draft.current = "";
    saved.current = "";
    setInitial(null);
    setLoadError(null);
    setSavedAt(null);
    statusRef.current({ state: "idle" });
    readCourseFile(file.relative_path)
      .then((t) => {
        if (!live) return;
        draft.current = t;
        saved.current = t;
        setWords(countWords(t));
        setInitial(t);
      })
      .catch((e) => live && setLoadError(String(e)));
    return () => {
      live = false;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [file.id]);

  // On leave, write anything unsaved; an in-flight save already loops on it.
  useEffect(() => {
    return () => {
      if (timer.current != null) window.clearTimeout(timer.current);
      timer.current = null;
      if (!inFlight.current && draft.current !== saved.current) {
        saveDocument(fileRef.current, draft.current).catch(console.error);
      }
    };
  }, [file.id]);

  // Pictures resolve against the note's folder, which a rename keeps.
  const dataDir = useDataDir();
  const noteDir = file.relative_path.replace(/[^/]+$/, "");
  const host = useMemo<NoteHost>(
    () => ({
      imageSrc: (src) => libraryImageSrc(src, noteDir, dataDir),
      openLink: (href) => openNoteLink(href, filesRef.current),
      subjectId: file.subject_id,
      // Read when `@` searches, so a rename needn't reconfigure the host.
      get notePath() {
        return fileRef.current.relative_path;
      },
    }),
    [noteDir, dataDir, file.subject_id],
  );
  const hostRef = useRef(host);
  hostRef.current = host;

  /**
   * Write a picture beside the note immediately and link it at the caret.
   * A note has no send to defer to, so a deleted tag leaves a file in
   * `assets/` — accepted over a dangling image. Undoable like typing.
   */
  const embed = useCallback(async (write: (note: DbFile) => Promise<string>, name: string) => {
    try {
      const path = await write(fileRef.current);
      const v = viewRef.current;
      if (!v) return;
      // Strip chars that would end the alt text early; Rust names the path.
      const alt = name.replace(/[[\]()]/g, "").trim() || "image";
      insertImage(`![${alt}](${path})`)(v);
      v.focus();
      setAttachError(null);
    } catch (e) {
      setAttachError(String(e));
    }
  }, []);

  /** Pasted pictures, one at a time to keep order. */
  const pastePictures = useCallback(
    (pictures: File[]) => {
      void (async () => {
        for (const picture of pictures) {
          await embed((note) => attachDocumentImage(note, picture), picture.name || "Pasted image");
        }
      })();
      return true;
    },
    [embed],
  );

  // The view lives as long as the loaded text: a file switch replaces it, a
  // rename does not. Creating it writes nothing — only an edit schedules.
  useEffect(() => {
    if (initial === null || !editorRef.current) return;
    const onUpdate = (u: ViewUpdate) => {
      if (u.docChanged) {
        draft.current = u.state.doc.toString();
        schedule();
      }
      if (u.focusChanged && !u.view.hasFocus) void flush();
      if (u.docChanged || u.selectionSet || syntaxTree(u.state) !== syntaxTree(u.startState)) {
        const next = activeFormats(u.state);
        setActive((prev) => (sameFormats(prev, next) ? prev : next));
      }
    };
    const v = new EditorView({
      parent: editorRef.current,
      state: EditorState.create({
        doc: initial,
        extensions: [
          noteExtensions({
            live: modeRef.current === "live",
            host: hostRef.current,
            suggest: suggestExtension(suggestionsRef.current, suggestConfig),
            onUpdate,
            onPictures: pastePictures,
          }),
          // The page scrolls under the sticky toolbar: a drag-select over it
          // scrolls up, and the caret is never scrolled in behind it.
          EditorView.scrollMargins.of(() => ({ top: toolbarRef.current?.offsetHeight ?? 0 })),
        ],
      }),
    });
    viewRef.current = v;
    setView(v);
    setActive(activeFormats(v.state));
    return () => {
      v.destroy();
      viewRef.current = null;
      setView(null);
    };
  }, [initial, schedule, flush, pastePictures, suggestConfig]);

  // Off destroys the plugin, which cancels anything in flight.
  useEffect(() => {
    viewRef.current?.dispatch({
      effects: suggestCompartment.reconfigure(suggestExtension(suggestions, suggestConfig)),
    });
  }, [suggestions, suggestConfig]);

  useEffect(() => {
    viewRef.current?.dispatch({ effects: hostCompartment.reconfigure(noteHost.of(host)) });
  }, [host]);

  useEffect(() => {
    const v = viewRef.current;
    if (!v) return;
    v.dispatch({ effects: liveCompartment.reconfigure(modeExtension(mode === "live")) });
    syncLiveFocus(v);
  }, [mode]);

  // Follow a rename unless the title field is being edited.
  useEffect(() => {
    if (document.activeElement !== titleRef.current) setTitle(fileTitle(file));
  }, [file.filename, file.category]); // eslint-disable-line react-hooks/exhaustive-deps

  // A new note opens on its title, selected, so typing replaces "Untitled".
  const loaded = initial !== null;
  useEffect(() => {
    if (!loaded || !UNTITLED.test(fileTitle(fileRef.current))) return;
    titleRef.current?.focus();
    titleRef.current?.select();
  }, [loaded]);

  // ⌘S / ⌘⇧P on the document, so they work from the title field too.
  useEffect(() => {
    if (!tabActive) return;
    const onKey = (e: KeyboardEvent) => {
      if (!(e.metaKey || e.ctrlKey) || e.altKey) return;
      const key = e.key.toLowerCase();
      if (key === "s" && !e.shiftKey) {
        e.preventDefault();
        void flush();
      } else if (key === "p" && e.shiftKey) {
        e.preventDefault();
        onMode(mode === "live" ? "raw" : "live");
      }
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [tabActive, flush, mode, onMode]);

  const commitTitle = useCallback(async () => {
    const next = title.trim();
    const current = fileTitle(fileRef.current);
    if (!next || next === current) {
      setTitle(current);
      return;
    }
    try {
      await flush();
      const path = await renameDocument(fileRef.current, next);
      // Until the row reloads, saves must go to the new path.
      fileRef.current = {
        ...fileRef.current,
        relative_path: path,
        filename: path.slice(path.lastIndexOf("/") + 1),
      };
      setTitle(fileTitle(fileRef.current));
    } catch (e) {
      statusRef.current({ state: "error", message: String(e) });
      setTitle(current);
    }
  }, [title, flush]);

  const onTitleKeyDown = (e: ReactKeyboardEvent<HTMLInputElement>) => {
    if (e.key === "Enter") {
      e.preventDefault();
      // Moving focus blurs the field, and the blur is what commits.
      viewRef.current?.focus();
    } else if (e.key === "Escape") {
      setTitle(fileTitle(fileRef.current));
      e.currentTarget.blur();
    }
  };

  /** Dropped or picked paths. A non-image says so rather than silently failing. */
  const attachPaths = (paths: string[]) => {
    if (!paths.length) return;
    const pictures = imagePaths(paths);
    if (!pictures.length) {
      setAttachError("Only images can go in a note.");
      return;
    }
    void (async () => {
      for (const path of pictures) {
        await embed(
          (note) => attachDocumentFile(note, path),
          path.slice(path.lastIndexOf("/") + 1),
        );
      }
    })();
  };

  const pickImages = () => {
    pickDocumentImages()
      .then(attachPaths)
      .catch((e) => setAttachError(String(e)));
  };

  const dropping = useFileDrop(pageRef, attachPaths);

  if (loadError) {
    return (
      <div className="px-6 py-5">
        <Alert variant="destructive">
          <AlertDescription className="text-xs">Failed to load file: {loadError}</AlertDescription>
        </Alert>
      </div>
    );
  }
  if (initial === null) {
    return <LoadingFill />;
  }

  return (
    <div ref={pageRef} className="relative h-full min-h-0">
      <div className="page-scroll">
        <div className="mx-auto w-full max-w-3xl px-6 py-8">
          <input
            ref={titleRef}
            value={title}
            onChange={(e) => setTitle(e.target.value)}
            onBlur={() => void commitTitle()}
            onKeyDown={onTitleKeyDown}
            placeholder="Untitled"
            aria-label="Title"
            spellCheck={false}
            className="block w-full border-0 bg-transparent p-0 font-display text-[22px] font-semibold leading-tight text-foreground outline-none placeholder:text-muted-foreground/40"
          />
          <DocumentMeta file={file} savedAt={savedAt} words={words} />
          {attachError && (
            <p className="mt-3 text-[11px] text-destructive">{attachError}</p>
          )}
          <Toolbar ref={toolbarRef} view={view} active={active} onImage={pickImages} />
          <div ref={editorRef} className="mt-4" />
        </div>
      </div>

      {/* Overlay over the whole note, since that is the drop target. */}
      <div
        aria-hidden
        className={cn(
          "pointer-events-none absolute inset-3 flex items-center justify-center rounded-xl border-2 border-dashed border-brand bg-brand/5 transition-opacity duration-150",
          dropping ? "opacity-100" : "opacity-0",
        )}
      >
        <span className="rounded-full bg-card px-3 py-1.5 text-[13px] font-medium text-brand shadow-sm">
          Drop to add a picture
        </span>
      </div>
    </div>
  );
}

/** Runs of letters or digits, so markdown's `#`, `-` and `*` don't count;
 *  a leading frontmatter block is properties, not prose. */
function countWords(text: string): number {
  const body = text.replace(/^---\r?\n[\s\S]*?\r?\n(?:---|\.\.\.)[ \t]*(?:\r?\n|$)/, "");
  return body.match(/[\p{L}\p{N}][\p{L}\p{N}'’_-]*/gu)?.length ?? 0;
}

/**
 * Subject · created · last updated · words, under the title. Created is the
 * row's first sighting (`first_seen_at`: the app's own create, or when
 * `reconcileDocuments` found a note written elsewhere). Updated is the later
 * of the row's `modified_at` — bumped on every save and on an outside edit —
 * and this mount's own last save; a note never edited has none.
 */
function DocumentMeta({
  file,
  savedAt,
  words,
}: {
  file: DbFile;
  savedAt: number | null;
  words: number;
}) {
  const now = useNow();
  const { subjects } = useSubjects();
  const subject = subjects.find((s) => s.id === file.subject_id) ?? null;
  const created = sqliteUtcToMs(file.first_seen_at) ?? sqliteUtcToMs(file.scraped_at);
  const rowUpdated = sqliteUtcToMs(file.modified_at);
  const updated =
    savedAt != null && (rowUpdated == null || savedAt > rowUpdated) ? savedAt : rowUpdated;

  const items: ReactNode[] = [];
  if (subject) {
    const href = `/subjects/${subject.id}`;
    items.push(
      <button
        key="subject"
        type="button"
        data-tab-href={href}
        onClick={() => navigateActive(href)}
        title={displayCode(subject.code)}
        className="min-w-0 cursor-pointer truncate transition-colors hover:text-foreground"
      >
        {displayName(subject.name, subject.code)}
      </button>,
    );
  }
  if (created != null) {
    items.push(
      <span key="created" title={fmtFullStamp(created)} className="shrink-0">
        Created {fmtRecent(created, now)}
      </span>,
    );
  }
  if (updated != null) {
    items.push(
      <span key="updated" title={fmtFullStamp(updated)} className="shrink-0">
        Last updated {fmtRecent(updated, now)}
      </span>,
    );
  }
  items.push(
    <span key="words" className="shrink-0 tabular-nums">
      {words.toLocaleString()} {words === 1 ? "word" : "words"}
    </span>,
  );

  return (
    <div className="mt-1.5 flex min-w-0 items-center gap-1.5 text-[12px] text-muted-foreground">
      {items.map((item, i) => (
        <Fragment key={isValidElement(item) ? item.key : i}>
          {i > 0 && (
            <span aria-hidden className="shrink-0 text-muted-foreground/50">
              ·
            </span>
          )}
          {item}
        </Fragment>
      ))}
    </div>
  );
}
