import { useRef, type RefObject } from "react";
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
 * one serialised write loop saves for all of them. Saved versions
 * (`@/lib/notes/documentVersions`) dock beside the note in `HistoryPanel`.
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
            <div ref={editorRef} data-selectable className="mt-4" />
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
