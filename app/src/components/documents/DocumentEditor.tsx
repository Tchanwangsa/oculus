import {
  useCallback,
  useEffect,
  useRef,
  useState,
  type ClipboardEvent as ReactClipboardEvent,
  type KeyboardEvent as ReactKeyboardEvent,
} from "react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import remarkMath from "remark-math";
import rehypeRaw from "rehype-raw";
import rehypeKatex from "rehype-katex";

import { Alert, AlertDescription } from "@/components/ui/alert";
import { PillTabs } from "@/components/ui/PillTabs";
import { useLibraryMdComponents } from "@/components/files/FileViewer";
import { normalizeMath } from "@/components/markdown/MdComponents";
import { useTabActive } from "@/components/tabs/TabContext";
import { useFileDrop } from "@/hooks/useFileDrop";
import { imageFiles, imagePaths } from "@/lib/attachments";
import {
  attachDocumentFile,
  attachDocumentImage,
  renameDocument,
  saveDocument,
} from "@/lib/documents";
import { applyEdit, enterEdit, imageEdit, tabEdit, type TextEdit } from "@/lib/markdownEditing";
import { fileTitle, openFileSmart } from "@/lib/openFile";
import { cn } from "@/lib/utils";
import type { DbFile } from "@/lib/db";
import { readCourseFile } from "@/lib/courseFiles";
import { LoadingFill } from "@/components/ui/PageParts";

export type EditorMode = "write" | "preview";

/** The header's status word; `idle` is blank. */
export type SaveStatus =
  | { state: "idle" }
  | { state: "saving" }
  | { state: "saved" }
  | { state: "error"; message: string };

const MODES = [
  { value: "write", label: "Write" },
  { value: "preview", label: "Preview" },
] as const;

/** How long after the last keystroke the draft goes to disk. */
const SAVE_DELAY_MS = 600;

/** A freshly created note, still wearing the name Rust gave it. */
const UNTITLED = /^Untitled(?:-\d+)?$/;

/** Write/Preview pills and the save word, drawn in the host page's header. */
export function DocumentControls({
  mode,
  onMode,
  status,
}: {
  mode: EditorMode;
  onMode: (mode: EditorMode) => void;
  status: SaveStatus;
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
      <PillTabs tabs={MODES} value={mode} onChange={onMode} />
    </div>
  );
}

/**
 * Apply a `TextEdit` via `execCommand` so ⌘Z undoes it like typing; its
 * `input` event updates React. If refused, `fallback` swaps the value in.
 */
function applyTextEdit(
  ta: HTMLTextAreaElement,
  edit: TextEdit,
  fallback: (value: string) => void,
) {
  // Nothing to do — and `delete` on a collapsed selection would eat a char.
  if (edit.start === edit.end && edit.text === "") return;
  ta.setSelectionRange(edit.start, edit.end);
  const done = edit.text
    ? document.execCommand("insertText", false, edit.text)
    : document.execCommand("delete");
  if (done) {
    ta.setSelectionRange(edit.caret, edit.caret);
    return;
  }
  fallback(applyEdit(ta.value, edit));
  requestAnimationFrame(() => ta.setSelectionRange(edit.caret, edit.caret));
}

/**
 * A markdown note, edited in place; controls live in the host's header.
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
}: {
  file: DbFile;
  /** The subject's files, for the preview to resolve `../` links against. */
  files: DbFile[];
  mode: EditorMode;
  onMode: (mode: EditorMode) => void;
  onStatus: (status: SaveStatus) => void;
}) {
  const tabActive = useTabActive();
  const [text, setText] = useState<string | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [title, setTitle] = useState(() => fileTitle(file));
  /** Picture-attach failures; separate from the save word, which the next
   *  keystroke would overwrite. */
  const [attachError, setAttachError] = useState<string | null>(null);

  const draft = useRef("");
  const saved = useRef("");
  const timer = useRef<number | null>(null);
  const inFlight = useRef<Promise<void> | null>(null);
  const bodyRef = useRef<HTMLTextAreaElement>(null);
  const titleRef = useRef<HTMLInputElement>(null);
  /** The whole page is the drop target, not just the text box. */
  const pageRef = useRef<HTMLDivElement>(null);

  // Updated after commit, not during render, so the cleanup on a file switch
  // (which React runs before new effects) still sees the old file.
  const fileRef = useRef(file);
  useEffect(() => {
    fileRef.current = file;
  });
  const statusRef = useRef(onStatus);
  statusRef.current = onStatus;

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
        statusRef.current({ state: "saving" });
        try {
          await saveDocument(fileRef.current, content);
          saved.current = content;
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
    setText(null);
    setLoadError(null);
    statusRef.current({ state: "idle" });
    readCourseFile(file.relative_path)
      .then((t) => {
        if (!live) return;
        draft.current = t;
        saved.current = t;
        setText(t);
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

  // Follow a rename unless the title field is being edited.
  useEffect(() => {
    if (document.activeElement !== titleRef.current) setTitle(fileTitle(file));
  }, [file.filename, file.category]); // eslint-disable-line react-hooks/exhaustive-deps

  // A new note opens on its title, selected, so typing replaces "Untitled".
  const loaded = text !== null;
  useEffect(() => {
    if (!loaded || !UNTITLED.test(fileTitle(fileRef.current))) return;
    titleRef.current?.focus();
    titleRef.current?.select();
  }, [loaded]);

  // Entering preview saves; leaving it focuses the text.
  const prevMode = useRef(mode);
  useEffect(() => {
    if (mode === "preview") void flush();
    else if (prevMode.current === "preview") bodyRef.current?.focus();
    prevMode.current = mode;
  }, [mode, flush]);

  // ⌘S / ⌘⇧P on the document, since in preview nothing here holds focus.
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
        onMode(mode === "write" ? "preview" : "write");
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
      bodyRef.current?.focus();
    } else if (e.key === "Escape") {
      setTitle(fileTitle(fileRef.current));
      e.currentTarget.blur();
    }
  };

  const onBodyKeyDown = (e: ReactKeyboardEvent<HTMLTextAreaElement>) => {
    if (e.nativeEvent.isComposing) return;
    const ta = e.currentTarget;
    let edit: TextEdit | null = null;
    if (e.key === "Tab" && !e.metaKey && !e.ctrlKey && !e.altKey) {
      edit = tabEdit(ta.value, ta.selectionStart, ta.selectionEnd, e.shiftKey);
    } else if (e.key === "Enter" && !e.metaKey && !e.ctrlKey && !e.altKey && !e.shiftKey) {
      edit = enterEdit(ta.value, ta.selectionStart, ta.selectionEnd);
    }
    if (!edit) return;
    e.preventDefault();
    applyTextEdit(ta, edit, (value) => {
      draft.current = value;
      setText(value);
      schedule();
    });
  };

  /**
   * Write a picture beside the note immediately and link it at the caret.
   * A note has no send to defer to, so a deleted tag leaves a file in
   * `assets/` — accepted over a dangling image. Undoable via `applyTextEdit`.
   */
  const embed = useCallback(
    async (write: (note: DbFile) => Promise<string>, name: string) => {
      try {
        const path = await write(fileRef.current);
        const ta = bodyRef.current;
        if (!ta) return;
        // Strip chars that would end the alt text early; Rust names the path.
        const alt = name.replace(/[[\]()]/g, "").trim() || "image";
        // A drop may leave focus elsewhere; the edit splices at this caret.
        ta.focus();
        const edit = imageEdit(
          ta.value,
          ta.selectionStart,
          ta.selectionEnd,
          `![${alt}](${path})`,
        );
        applyTextEdit(ta, edit, (value) => {
          draft.current = value;
          setText(value);
          schedule();
        });
        setAttachError(null);
      } catch (e) {
        setAttachError(String(e));
      }
    },
    [schedule],
  );

  /** Pasted pictures, one at a time to keep order. Picture beats a text flavour. */
  const onBodyPaste = (e: ReactClipboardEvent<HTMLTextAreaElement>) => {
    const pictures = imageFiles(e.clipboardData?.files);
    if (!pictures.length) return;
    e.preventDefault();
    void (async () => {
      for (const picture of pictures) {
        await embed((note) => attachDocumentImage(note, picture), picture.name || "Pasted image");
      }
    })();
  };

  /** Dropped paths. A non-image drop says so rather than silently failing. */
  const attachPaths = (paths: string[]) => {
    if (!paths.length) return;
    if (mode !== "write") {
      setAttachError("Switch to Write to add a picture.");
      return;
    }
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

  const dropping = useFileDrop(pageRef, attachPaths);

  const components = useLibraryMdComponents(file, files, openFileSmart);

  if (loadError) {
    return (
      <div className="px-6 py-5">
        <Alert variant="destructive">
          <AlertDescription className="text-xs">Failed to load file: {loadError}</AlertDescription>
        </Alert>
      </div>
    );
  }
  if (text === null) {
    return (
      <LoadingFill />
    );
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
          {attachError && (
            <p className="mt-3 text-[11px] text-destructive">{attachError}</p>
          )}
          {mode === "write" ? (
            <textarea
              ref={bodyRef}
              value={text}
              onChange={(e) => {
                draft.current = e.target.value;
                setText(e.target.value);
                schedule();
              }}
              onBlur={() => void flush()}
              onKeyDown={onBodyKeyDown}
              onPaste={onBodyPaste}
              placeholder="Start writing…"
              aria-label="Document text"
              className="mt-4 block min-h-[50vh] w-full resize-none border-0 bg-transparent p-0 text-[14px] leading-[1.7] text-foreground outline-none field-sizing-content placeholder:text-muted-foreground/40"
            />
          ) : text.trim() === "" ? (
            <p className="mt-4 text-[14px] leading-[1.7] text-muted-foreground/60">
              Nothing to preview yet.
            </p>
          ) : (
            <article className="markdown-body mt-4">
              <ReactMarkdown
                remarkPlugins={[remarkGfm, remarkMath]}
                rehypePlugins={[rehypeRaw, rehypeKatex]}
                components={components}
              >
                {normalizeMath(text)}
              </ReactMarkdown>
            </article>
          )}
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
