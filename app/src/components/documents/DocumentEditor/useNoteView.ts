import { useEffect, useRef, useState, type RefObject } from "react";
import { syntaxTree } from "@codemirror/language";
import { EditorView, type ViewUpdate } from "@codemirror/view";

import { registerNoteLinkCommand } from "@/lib/notes/noteShortcuts";
import type { DocumentLease } from "@/lib/notes/documentSessions";
import type { DbFile } from "@/lib/db";

import type { EditorMode } from "../DocumentControls";
import { suggestCompartment, suggestExtension, type SuggestConfig } from "../editor/chrome/aiSuggest";
import { activeFormats, NO_FORMATS, sameFormats, toggleLink, type ActiveFormats } from "../editor/commands";
import { liveCompartment, modeExtension, noteExtensions } from "../editor/core/extensions";
import { hostCompartment, noteHost, type NoteHost } from "../editor/core/host";
import { syncLiveFocus } from "../editor/live-preview/livePreview";

/**
 * The CodeMirror view: made once the session's text has loaded, reconfigured
 * as the suggestion toggle, host and mode change, and destroyed with it.
 * Returns the view and the formats at its selection, for the toolbar.
 */
export function useNoteView({
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
}: {
  loadedId: number | null;
  leaseRef: RefObject<DocumentLease<DbFile> | null>;
  viewRef: RefObject<EditorView | null>;
  editorRef: RefObject<HTMLDivElement | null>;
  toolbarRef: RefObject<HTMLDivElement | null>;
  mode: EditorMode;
  host: NoteHost;
  hostRef: RefObject<NoteHost>;
  suggestions: boolean;
  suggestionsRef: RefObject<boolean>;
  suggestConfig: SuggestConfig;
  pastePictures: (pictures: File[]) => boolean;
}) {
  const [view, setView] = useState<EditorView | null>(null);
  const [active, setActive] = useState<ActiveFormats>(NO_FORMATS);
  const modeRef = useRef(mode);
  modeRef.current = mode;

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
  }, [loadedId, pastePictures, suggestConfig, leaseRef, viewRef, editorRef, toolbarRef, hostRef, suggestionsRef]);

  // Off destroys the plugin, which cancels anything in flight.
  useEffect(() => {
    viewRef.current?.dispatch({
      effects: suggestCompartment.reconfigure(suggestExtension(suggestions, suggestConfig)),
    });
  }, [suggestions, suggestConfig, viewRef]);

  useEffect(() => {
    viewRef.current?.dispatch({ effects: hostCompartment.reconfigure(noteHost.of(host)) });
  }, [host, viewRef]);

  useEffect(() => {
    const v = viewRef.current;
    if (!v) return;
    v.dispatch({ effects: liveCompartment.reconfigure(modeExtension(mode === "live")) });
    syncLiveFocus(v);
  }, [mode, viewRef]);

  return { view, active };
}
