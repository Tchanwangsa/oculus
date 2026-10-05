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
  type RefObject,
} from "react";
import { isolateHistory } from "@codemirror/commands";
import { syntaxTree } from "@codemirror/language";
import { EditorView, type ViewUpdate } from "@codemirror/view";

import { Alert, AlertDescription } from "@/components/ui/alert";
import { DropOverlay } from "@/components/ui/DropOverlay";
import { FindBar } from "@/components/ui/FindBar";
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
  suggestDocument,
} from "@/lib/documents";
import { documentSessions, type DocumentLease } from "@/lib/documentSessions";
import { saveCheckpoint } from "@/lib/documentVersions";
import {
  displayCode,
  displayName,
  fmtFullStamp,
  fmtRecent,
  sqliteUtcToMs,
} from "@/lib/format";
import { libraryImageSrc } from "@/lib/libraryLinks";
import { registerNoteLinkCommand } from "@/lib/noteShortcuts";
import { fileTitle, openNoteLink } from "@/lib/openFile";
import { navigateActive } from "@/lib/tabRouters";
import type { DbFile, Subject } from "@/lib/db";
import { LoadingFill } from "@/components/ui/PageParts";

import {
  activeFormats,
  insertImage,
  NO_FORMATS,
  sameFormats,
  toggleLink,
  type ActiveFormats,
} from "./editor/commands";
import {
  suggestCompartment,
  suggestExtension,
  type SuggestConfig,
} from "./editor/aiSuggest";
import { liveCompartment, modeExtension, noteExtensions } from "./editor/extensions";
import { hostCompartment, noteHost, type NoteHost } from "./editor/host";
import { syncLiveFocus } from "./editor/livePreview";
import { Toolbar } from "./editor/Toolbar";
import { useEditorFind } from "./editor/useEditorFind";
import { HistoryPanel } from "./HistoryPanel";
import type { DocumentActions, EditorMode, SaveStatus, SuggestStatus } from "./DocumentControls";

/** A freshly created note, still wearing the name Rust gave it. */
const UNTITLED = /^Untitled(?:-\d+)?$/;

/**
 * A markdown note, edited in place in CodeMirror (`./editor/`); controls live
 * in the host's header. The file's text is the document — nothing is
 * re-serialised. Live mode renders markdown around the caret, Raw is the same
 * editor without that.
 *
 * The text and its saves belong to the note's session (`@/lib/documentSessions`),
 * shared with every other editor of the same note: they stay identical, and
 * one serialised write loop saves for all of them. Saved versions
 * (`@/lib/documentVersions`) dock beside the note in `HistoryPanel`.
 */
export function DocumentEditor({
  file,
  files,
  mode,
  onMode,
  onStatus,
  suggestions,
  onSuggestStatus,
  history = false,
  onHistory,
  actions,
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
  /** The version history panel, opened from the host's header. */
  history?: boolean;
  onHistory?: (open: boolean) => void;
  /** Filled while mounted, for the header's Save version. */
  actions?: RefObject<DocumentActions | null>;
}) {
  const tabActive = useTabActive();
  /** The row whose text the session has; the view is made once it does. */
  const [loadedId, setLoadedId] = useState<number | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [title, setTitle] = useState(() => fileTitle(file));
  /** Picture-attach failures; separate from the save word, which the next
   *  keystroke would overwrite. */
  const [attachError, setAttachError] = useState<string | null>(null);
  const [view, setView] = useState<EditorView | null>(null);
  const [active, setActive] = useState<ActiveFormats>(NO_FORMATS);
  /** The session's last save, and the words on disk — both move on save,
   *  not per keystroke. The row's `modified_at` isn't re-read on save. */
  const [savedAt, setSavedAt] = useState<number | null>(null);
  const [words, setWords] = useState(0);

  const leaseRef = useRef<DocumentLease<DbFile> | null>(null);
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

  /** Write the note if it differs from disk. Idempotent. */
  const flush = useCallback(
    (): Promise<void> => leaseRef.current?.flush() ?? Promise.resolve(),
    [],
  );

  /** What the editor shows now: the view's text, else the session's. */
  const currentText = useCallback(
    (): string | null => viewRef.current?.state.doc.toString() ?? leaseRef.current?.text ?? null,
    [],
  );

  // Writes what is pending first, so the version and the file agree.
  useEffect(() => {
    if (!actions) return;
    const mine: DocumentActions = {
      saveVersion: async (label) => {
        await flush();
        const text = currentText();
        if (text == null) throw new Error("The note is still loading.");
        return saveCheckpoint(fileRef.current.id, text, label);
      },
    };
    actions.current = mine;
    return () => {
      if (actions.current === mine) actions.current = null;
    };
  }, [actions, flush, currentText]);

  /** A restored version replaces the whole text as one ordinary edit, kept
   *  out of neighbouring typing's undo step: it saves through the session,
   *  reaches the note's other views, and ⌘Z takes it back. */
  const replaceText = useCallback((text: string): boolean => {
    const v = viewRef.current;
    if (!v) return false;
    v.dispatch({
      changes: { from: 0, to: v.state.doc.length, insert: text },
      annotations: isolateHistory.of("full"),
    });
    return true;
  }, []);

  const { subjects } = useSubjects();
  const subject = subjects.find((s) => s.id === file.subject_id) ?? null;

  // Join the note's session, keyed on the row, not its path: a rename must
  // not reload the text. Leaving writes anything unsaved (the session does).
  useEffect(() => {
    let live = true;
    setLoadError(null);
    const lease = documentSessions.open(file, {
      status: (s) => statusRef.current(s),
      saved: (text, at) => {
        setSavedAt(at);
        setWords(countWords(text));
      },
    });
    leaseRef.current = lease;
    const ready = () => {
      setWords(countWords(lease.savedText));
      setSavedAt(lease.savedAt);
      setLoadedId(file.id);
    };
    if (lease.text !== null) ready();
    else {
      lease
        .load()
        .then(() => live && ready())
        .catch((e) => live && setLoadError(String(e)));
    }
    return () => {
      live = false;
      if (leaseRef.current === lease) leaseRef.current = null;
      lease.release();
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
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

  // The view lives as long as the session: a file switch replaces it, a
  // rename does not. It starts from the session's current text, so a second
  // pane shows the first one's unsaved typing. Only a local edit schedules.
  useEffect(() => {
    const lease = leaseRef.current;
    const text = lease?.text;
    if (loadedId === null || !lease || text == null || !editorRef.current) return;
    const onUpdate = (u: ViewUpdate) => {
      if (u.docChanged) lease.changed(u.view, u.transactions);
      if (u.focusChanged && !u.view.hasFocus) void lease.flush();
      if (u.docChanged || u.selectionSet || syntaxTree(u.state) !== syntaxTree(u.startState)) {
        const next = activeFormats(u.state);
        setActive((prev) => (sameFormats(prev, next) ? prev : next));
      }
    };
    const v = new EditorView({
      parent: editorRef.current,
      state: lease.restore({
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
    const unbind = lease.bind(v);
    const unregisterLink = registerNoteLinkCommand(v.dom, () => toggleLink(v));
    viewRef.current = v;
    setView(v);
    setActive(activeFormats(v.state));
    return () => {
      unbind();
      unregisterLink();
      v.destroy();
      viewRef.current = null;
      setView(null);
    };
  }, [loadedId, pastePictures, suggestConfig]);

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
  const loaded = loadedId !== null;
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
    const lease = leaseRef.current;
    if (!lease) {
      setTitle(current);
      return;
    }
    try {
      // A failed save keeps its error status and must not move the unsaved note.
      const moved = await lease.rename(async (note) => {
        const path = await renameDocument(note, next);
        return { ...note, relative_path: path, filename: path.slice(path.lastIndexOf("/") + 1) };
      });
      if (!moved) {
        setTitle(current);
        return;
      }
      // Until the row reloads, suggestions and `@` search use the new path.
      fileRef.current = {
        ...fileRef.current,
        relative_path: lease.file.relative_path,
        filename: lease.file.filename,
      };
      setTitle(fileTitle(fileRef.current));
    } catch (e) {
      statusRef.current({ state: "error", message: String(e) });
      setTitle(current);
    }
  }, [title]);

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
  // ⌘F anywhere on the page, title included, searches the note.
  const find = useEditorFind(view, pageRef);

  if (loadError) {
    return (
      <div className="px-6 py-5">
        <Alert variant="destructive">
          <AlertDescription className="text-xs">Failed to load file: {loadError}</AlertDescription>
        </Alert>
      </div>
    );
  }
  if (loadedId === null) {
    return <LoadingFill />;
  }

  return (
    <div className="flex h-full min-h-0">
      <div ref={pageRef} className="relative min-w-0 flex-1">
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
            <DocumentMeta file={file} subject={subject} savedAt={savedAt} words={words} />
            {attachError && (
              <p className="mt-3 text-[11px] text-destructive">{attachError}</p>
            )}
            {/* The find row sticks with the toolbar; both are the caret's top margin. */}
            <div ref={toolbarRef} className="sticky top-0 z-10 -mx-2 mt-3 bg-card">
              <Toolbar
                view={view}
                active={active}
                onImage={pickImages}
                className="static z-auto mx-0 mt-0"
              />
              {find.open && (
                <FindBar
                  {...find.bar}
                  placeholder="Find in document"
                  className="border-b border-border-subtle"
                />
              )}
            </div>
            <div ref={editorRef} className="mt-4" />
          </div>
        </div>

        {/* Over the whole note, since that is the drop target. */}
        <DropOverlay show={dropping} label="Drop to add a picture" />
      </div>
      {history && (
        <HistoryPanel
          file={file}
          files={files}
          subject={subject}
          currentText={currentText}
          replaceText={replaceText}
          onClose={() => onHistory?.(false)}
        />
      )}
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
 * and the session's last save; a note never edited has none.
 */
function DocumentMeta({
  file,
  subject,
  savedAt,
  words,
}: {
  file: DbFile;
  subject: Subject | null;
  savedAt: number | null;
  words: number;
}) {
  const now = useNow();
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
