import { useEffect, useRef, useState, type RefObject } from "react";
import type { EditorView } from "@codemirror/view";

import { Alert, AlertDescription } from "@/components/ui/alert";
import { DropOverlay } from "@/components/ui/layout/DropOverlay";
import { LoadingFill } from "@/components/ui/layout/PageParts";
import { FindBar } from "@/components/ui/search/FindBar";
import { useTabActive } from "@/components/tabs/TabContext";
import { useFileDrop } from "@/hooks/gestures/useFileDrop";
import { useSubjects } from "@/hooks/data/useSubjects";
import type { DbFile } from "@/lib/db";

import { Toolbar } from "../editor/chrome/Toolbar";
import { useEditorFind } from "../editor/chrome/useEditorFind";
import { HistoryPanel } from "../HistoryPanel";
import { NoteColumn } from "../NoteColumn";
import type { DocumentActions, EditorMode, SaveStatus, SuggestStatus } from "../DocumentControls";
import { DocumentMeta } from "./DocumentMeta";
import { useNoteHost } from "./useNoteHost";
import { useNotePictures } from "./useNotePictures";
import { useNoteSession } from "./useNoteSession";
import { useNoteShortcuts } from "./useNoteShortcuts";
import { useNoteSuggest } from "./useNoteSuggest";
import { useNoteTitle } from "./useNoteTitle";
import { useNoteView } from "./useNoteView";

/**
 * A markdown note, edited in place in CodeMirror (`../editor/`); controls live
 * in the host's header. The file's text is the document — nothing is
 * re-serialised. Live mode renders markdown around the caret, Raw is the same
 * editor without that.
 *
 * The text and its saves belong to the note's session (`@/lib/notes/documentSessions`),
 * shared with every other editor of the same note: they stay identical, and
 * one serialised write loop saves for all of them. While History is open
 * (`HistoryPanel`) a saved version takes the note's place on the page; the
 * note stays mounted underneath, hidden and inert, so its scroll position and
 * caret are as they were when History closes.
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
  /** Inline AI suggestions (`../editor/chrome/aiSuggest.ts`) on or off. */
  suggestions: boolean;
  onSuggestStatus: (status: SuggestStatus) => void;
  /** The version history panel, opened from the host's header. */
  history?: boolean;
  onHistory?: (open: boolean) => void;
  /** Filled while mounted, for the header's Save version. */
  actions?: RefObject<DocumentActions | null>;
}) {
  const tabActive = useTabActive();
  const viewRef = useRef<EditorView | null>(null);
  const editorRef = useRef<HTMLDivElement>(null);
  const toolbarRef = useRef<HTMLDivElement>(null);
  /** The whole page is the drop target, not just the text. */
  const pageRef = useRef<HTMLDivElement>(null);
  /** The note and History over it: the find target. */
  const rootRef = useRef<HTMLDivElement>(null);
  /** The version on the page, when History shows one. */
  const [versionView, setVersionView] = useState<EditorView | null>(null);

  const { fileRef, statusRef, leaseRef, loadedId, loadError, savedAt, words, flush, currentText, replaceText } =
    useNoteSession(file, onStatus, actions, viewRef);
  const { subjects } = useSubjects();
  const subject = subjects.find((s) => s.id === file.subject_id) ?? null;
  const { host, hostRef } = useNoteHost(file, files, fileRef);
  const { attachError, pastePictures, attachPaths, pickImages } = useNotePictures(fileRef, viewRef);
  const { suggestionsRef, suggestConfig } = useNoteSuggest(fileRef, suggestions, onSuggestStatus);
  const { view, active } = useNoteView({
    loadedId,
    leaseRef,
    viewRef,
    editorRef,
    toolbarRef,
    mode,
    host,
    hostRef,
    suggestions,
    suggestionsRef,
    suggestConfig,
    pastePictures,
  });
  const { title, setTitle, titleRef, commitTitle, onTitleKeyDown } = useNoteTitle({
    file,
    fileRef,
    leaseRef,
    statusRef,
    viewRef,
    loaded: loadedId !== null,
  });
  useNoteShortcuts(tabActive, flush, mode, onMode);

  // Pictures dropped while History is open would land in the hidden note.
  const dropping = useFileDrop(pageRef, (paths) => {
    if (!history) attachPaths(paths);
  });
  // ⌘F anywhere on the page, title included, searches the note. While History
  // is open it searches the version on the page, or nothing — never the
  // hidden note.
  const find = useEditorFind(history ? versionView : view, rootRef);
  const findBar = find.open && (
    <FindBar
      {...find.bar}
      placeholder={history ? "Find in version" : "Find in document"}
      className="border-b border-border-subtle"
    />
  );

  // Back from History: CodeMirror re-measures what changed while hidden, and
  // focus that went with History's controls returns to the note.
  const wasHistory = useRef(false);
  useEffect(() => {
    const back = wasHistory.current && !history;
    wasHistory.current = history;
    if (!back || !view) return;
    view.requestMeasure();
    if (!document.activeElement || document.activeElement === document.body) view.focus();
  }, [history, view]);

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
    <div ref={rootRef} className="relative flex h-full min-h-0">
      <div
        ref={pageRef}
        // Hidden, never unmounted or `display: none`, so scroll and caret hold.
        style={history ? { visibility: "hidden" } : undefined}
        inert={history}
        className="relative min-w-0 flex-1"
      >
        <NoteColumn>
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
              getView={() => view}
              active={active}
              onImage={pickImages}
              className="static z-auto mx-0 mt-0"
            />
            {!history && findBar}
          </div>
          <div ref={editorRef} data-selectable className="mt-4" />
        </NoteColumn>

        {/* Over the whole note, since that is the drop target. */}
        <DropOverlay show={dropping && !history} label="Drop to add a picture" />
      </div>
      {history && (
        <HistoryPanel
          file={file}
          subject={subject}
          title={title}
          host={host}
          currentText={currentText}
          replaceText={replaceText}
          findBar={findBar}
          onView={setVersionView}
          onClose={() => onHistory?.(false)}
        />
      )}
    </div>
  );
}
