import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { syntaxTree } from "@codemirror/language";
import { Prec, Transaction } from "@codemirror/state";
import { EditorView, keymap, type ViewUpdate } from "@codemirror/view";

import { FindBar } from "@/components/ui/search/FindBar";
import { useFileDrop } from "@/hooks/gestures/useFileDrop";
import { useScrollFade } from "@/hooks/ui/useScrollFade";
import { imagePaths } from "@/lib/harness/attachments";
import { pickDocumentImages } from "@/lib/notes/documents";
import { registerNoteLinkCommand } from "@/lib/notes/noteShortcuts";
import { openNoteLink } from "@/lib/files/openFile";
import { cn } from "@/lib/utils";

import {
  activeFormats,
  insertImage,
  NO_FORMATS,
  sameFormats,
  toggleLink,
  type ActiveFormats,
} from "./editor/commands";
import { noteExtensions } from "./editor/core/extensions";
import { hostCompartment, noteHost, type NoteHost } from "./editor/core/host";
import { ViewSlot } from "./editor/core/viewSlot";
import { Toolbar } from "./editor/chrome/Toolbar";
import { useEditorFind } from "./editor/chrome/useEditorFind";

/** The document page's `.cm-content` is half a screen tall; a field is a
 *  few lines. Highest precedence: CodeMirror mounts it after `noteTheme`. */
const fieldTheme = Prec.highest(EditorView.theme({ ".cm-content": { minHeight: "6rem" } }));

/**
 * A markdown field on the note editor (`./editor/`) in Live mode — a short
 * text kept in a database row rather than a note file, so no session, title,
 * Raw mode or AI suggestions. At rest it reads as prose; while edited it has
 * a brand border and the `Toolbar` above the text. The toolbar waits for a
 * press that focused the field to end, so the line being clicked or dragged
 * over never moves under the pointer.
 *
 * The text is the caller's: `onCommit` gets it when focus leaves, on ⌘↵ and
 * on unmount, but only if edited since `text` last landed. A new `text` (an
 * outside write) replaces the doc while the field is at rest and waits while
 * it is edited. Key the field by its row, so another row starts a fresh doc.
 * `onChange` hears every change at once, for a caller whose buttons read the
 * text (a dialog) and so can't wait for the blur.
 *
 * Pictures (pasted, dropped, picked) go through `writePicture` the moment
 * they arrive and are linked at the caret.
 */
export function NoteField({
  text,
  subjectId,
  imageSrc,
  writePicture,
  pickerTitle,
  notAPicture,
  placeholder,
  label,
  onCommit,
  onChange,
  className,
  scrollClassName,
}: {
  /** The stored text; read at mount, then followed while at rest. */
  text: string;
  /** `@` searches this subject, or the whole library when null. */
  subjectId: number | null;
  /** What an `<img>` loads for a picture's markdown `src`. */
  imageSrc: (src: string) => string;
  /** Writes one pasted `File` or picked/dropped path; returns the path to link. */
  writePicture: (source: File | string) => Promise<string>;
  /** The image picker's title. */
  pickerTitle: string;
  /** Said when a drop holds no picture. */
  notAPicture: string;
  placeholder: string;
  label: string;
  onCommit: (text: string) => void;
  /** The doc after each change, edits and outside writes alike. */
  onChange?: (text: string) => void;
  /** Placement and margins, on the outermost element. */
  className?: string;
  /** The text scroller's height bound, in place of 420px. */
  scrollClassName?: string;
}) {
  const [view, setView] = useState<EditorView | null>(null);
  const [formats, setFormats] = useState<ActiveFormats>(NO_FORMATS);
  const [editing, setEditing] = useState(false);
  /** A press that began on a field at rest is down; the toolbar waits for it. */
  const [pressing, setPressing] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const slot = useMemo(() => new ViewSlot<EditorView>(), []);
  const releasePress = useRef<(() => void) | null>(null);
  const rootRef = useRef<HTMLDivElement>(null);
  const cardRef = useRef<HTMLDivElement>(null);
  const scrollRef = useRef<HTMLDivElement>(null);
  const editorRef = useRef<HTMLDivElement>(null);
  const viewRef = useRef<EditorView | null>(null);
  /** The text the doc last matched: the stored text or the last commit. */
  const baseRef = useRef(text);
  const editingRef = useRef(false);
  const leaveTimer = useRef<number | null>(null);

  const textRef = useRef(text);
  textRef.current = text;
  const commitRef = useRef(onCommit);
  commitRef.current = onCommit;
  const changeRef = useRef(onChange);
  changeRef.current = onChange;
  const writeRef = useRef(writePicture);
  writeRef.current = writePicture;

  const host = useMemo<NoteHost>(
    () => ({
      imageSrc,
      // No file list: a library path or web URL opens; `../` links have no base.
      openLink: (href) => openNoteLink(href, []),
      subjectId,
      notePath: null,
    }),
    [imageSrc, subjectId],
  );
  const hostRef = useRef(host);
  hostRef.current = host;

  /** Hand an edited doc to the caller; false when there was no edit. */
  const commit = useCallback((): boolean => {
    const doc = viewRef.current?.state.doc.toString();
    if (doc == null || doc === baseRef.current) return false;
    baseRef.current = doc;
    commitRef.current(doc);
    return true;
  }, []);

  /** Take the stored text unless the user has edits it would overwrite. Not
   *  an undo step: the user didn't type it. */
  const follow = useCallback(() => {
    const v = viewRef.current;
    const next = textRef.current;
    const doc = v?.state.doc.toString();
    if (!v || doc !== baseRef.current) return;
    baseRef.current = next;
    if (doc === next) return;
    v.dispatch({
      changes: { from: 0, to: v.state.doc.length, insert: next },
      annotations: Transaction.addToHistory.of(false),
    });
  }, []);

  /** Write a picture and link it at the caret. A field has no send to defer
   *  to, so a picture later deleted from the text leaves its file behind. */
  const embed = useCallback(async (source: File | string) => {
    const name =
      typeof source === "string"
        ? source.slice(source.lastIndexOf("/") + 1)
        : source.name || "Pasted image";
    try {
      const path = await writeRef.current(source);
      const v = viewRef.current;
      if (!v) return;
      // Strip chars that would end the alt text early; Rust names the path.
      const alt = name.replace(/[[\]()]/g, "").trim() || "image";
      insertImage(`![${alt}](${path})`)(v);
      v.focus();
      setError(null);
    } catch (e) {
      setError(String(e));
    }
  }, []);

  /** Dropped or picked paths, one at a time to keep order. */
  const attachPaths = (paths: string[]) => {
    if (!paths.length) return;
    const pictures = imagePaths(paths);
    if (!pictures.length) {
      setError(notAPicture);
      return;
    }
    void (async () => {
      for (const p of pictures) await embed(p);
    })();
  };

  // Mount before paint: replacing rendered prose with an empty field would
  // briefly shrink the chat and clamp its scroll position to the bottom.
  useLayoutEffect(() => {
    const parent = editorRef.current;
    if (!parent) return;
    const onUpdate = (u: ViewUpdate) => {
      if (u.docChanged) changeRef.current?.(u.state.doc.toString());
      if (u.docChanged || u.selectionSet || syntaxTree(u.state) !== syntaxTree(u.startState)) {
        const next = activeFormats(u.state);
        setFormats((prev) => (sameFormats(prev, next) ? prev : next));
      }
    };
    const pastePictures = (pictures: File[]) => {
      void (async () => {
        for (const picture of pictures) await embed(picture);
      })();
      return true;
    };
    baseRef.current = textRef.current;
    // The completion list and maths popover leave the scroller below, which
    // would clip and mask them; a modal keeps its own, since a body-level
    // tooltip is outside the modal's reach.
    const tooltipParent =
      parent.closest<HTMLElement>('[role="dialog"]') ?? parent.ownerDocument.body;
    // Exactly one view lives in the host, whatever the effect's history.
    parent.replaceChildren();
    const v = slot.mount(
      () =>
        new EditorView({
          parent,
          doc: textRef.current,
          extensions: [
            noteExtensions({
              live: true,
              host: hostRef.current,
              suggest: [],
              onUpdate,
              onPictures: pastePictures,
              placeholder,
              label,
              tooltipParent,
            }),
            fieldTheme,
            Prec.highest(
              keymap.of([
                {
                  key: "Mod-Enter",
                  run: () => {
                    commit();
                    return true;
                  },
                },
              ]),
            ),
          ],
        }),
    );
    const unregisterLink = registerNoteLinkCommand(v.dom, () => toggleLink(v));
    viewRef.current = v;
    setView(v);
    setFormats(activeFormats(v.state));
    return () => {
      try {
        // Leaving the row (a key change, the page closing) saves what was typed.
        commit();
        unregisterLink();
      } finally {
        slot.unmount(v);
        viewRef.current = null;
        setView(null);
      }
    };
    // Placeholder and label are fixed for a field's life.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [commit, embed, slot]);

  useEffect(() => {
    viewRef.current?.dispatch({ effects: hostCompartment.reconfigure(noteHost.of(host)) });
  }, [host]);

  // An outside write lands now if the field is at rest, else on leaving it.
  useEffect(() => {
    if (!editingRef.current) follow();
  }, [text, follow]);

  useEffect(
    () => () => {
      if (leaveTimer.current != null) window.clearTimeout(leaveTimer.current);
      releasePress.current?.();
    },
    [],
  );

  /** The toolbar mounts above the text and pushes it down, so a press that
   *  gives a field at rest its focus holds the toolbar back until it ends:
   *  the caret lands, or a drag selects, on text that has not yet moved. */
  const onPointerDown = () => {
    if (editingRef.current || releasePress.current) return;
    const release = () => {
      window.removeEventListener("pointerup", release);
      window.removeEventListener("pointercancel", release);
      releasePress.current = null;
      setPressing(false);
    };
    window.addEventListener("pointerup", release);
    window.addEventListener("pointercancel", release);
    releasePress.current = release;
    setPressing(true);
  };

  /**
   * Focus anywhere in the card's React tree counts as editing — that includes
   * the toolbar's portaled Select and table popover, whose focus events bubble
   * here through React. A blur is judged a tick later: focus moving into one
   * of those arrives first. Focus still in the card, or the window itself
   * losing focus (the picture picker, another app), is not leaving.
   */
  const onFocus = () => {
    if (leaveTimer.current != null) window.clearTimeout(leaveTimer.current);
    leaveTimer.current = null;
    editingRef.current = true;
    setEditing(true);
  };
  const onBlur = () => {
    if (leaveTimer.current != null) window.clearTimeout(leaveTimer.current);
    leaveTimer.current = window.setTimeout(() => {
      leaveTimer.current = null;
      if (!document.hasFocus() || cardRef.current?.contains(document.activeElement)) return;
      editingRef.current = false;
      setEditing(false);
      // A commit's own write comes back as `text`; only an unedited doc
      // takes a write that arrived while editing.
      if (!commit()) follow();
    }, 0);
  };

  const dropping = useFileDrop(rootRef, attachPaths);
  useScrollFade(scrollRef);
  const find = useEditorFind(view, rootRef);

  return (
    <div ref={rootRef} className={className}>
      {error && <div className="px-2 pb-1 text-[11px] text-destructive">{error}</div>}
      <div
        ref={cardRef}
        onFocus={onFocus}
        onBlur={onBlur}
        onPointerDown={onPointerDown}
        onMouseDown={(e) => {
          // The padding around the text edits it rather than dropping focus.
          if (e.target !== e.currentTarget && e.target !== scrollRef.current) return;
          e.preventDefault();
          viewRef.current?.focus();
        }}
        className={cn(
          "relative cursor-text rounded-lg border transition-colors",
          editing ? "border-brand/40 bg-card" : "border-transparent hover:border-border-subtle",
          dropping && "border-brand ring-[3px] ring-brand/25",
        )}
      >
        {/* In the card, so focus in the bar still counts as editing. */}
        {editing && !pressing && (
          <div
            onMouseDown={(e) => {
              // A click between the buttons must not blur the editor.
              if (e.currentTarget.contains(e.target as Node)) e.preventDefault();
            }}
          >
            <Toolbar
              getView={() => view}
              active={formats}
              onImage={() =>
                pickDocumentImages(pickerTitle)
                  .then(attachPaths)
                  .catch((e) => setError(String(e)))
              }
              className="static z-auto mx-0 mt-0 cursor-default rounded-t-lg"
            />
          </div>
        )}
        {/* Bounded so a long text scrolls inside and what follows stays reachable. */}
        <div
          ref={scrollRef}
          className={cn("max-h-[420px] overflow-x-hidden overflow-y-auto px-2 py-1.5", scrollClassName)}
        >
          <div ref={editorRef} data-selectable />
        </div>
        {/* Floats under the toolbar, at the card's top right. */}
        {find.open && (
          <FindBar
            {...find.bar}
            variant="floating"
            placeholder="Find in note"
            className="top-[3.25rem]"
          />
        )}
      </div>
    </div>
  );
}
