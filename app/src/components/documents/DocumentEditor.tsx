import {
  useCallback,
  useEffect,
  useRef,
  useState,
  type ClipboardEvent as ReactClipboardEvent,
  type KeyboardEvent as ReactKeyboardEvent,
} from "react";
import { invoke } from "@tauri-apps/api/core";
import { CircleNotch } from "@phosphor-icons/react";
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

export type EditorMode = "write" | "preview";

/** What the header's status word says. `idle` is blank: nothing has been
 *  typed since the note was opened, or since it was last saved and then
 *  touched again — a word for every keystroke would be noise. */
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

/**
 * The editor's header controls — the Write / Preview pills and the save
 * word — drawn by the page that hosts the editor, in *its* header, so the
 * document below stays chrome-free.
 */
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
 * Turn a `TextEdit` into an insertion the browser makes itself, so ⌘Z takes
 * it back like any other typing. The `input` event it raises is what updates
 * React's copy; only when the command is refused is the value swapped in by
 * hand, with the caret placed once that render has landed.
 */
function applyTextEdit(
  ta: HTMLTextAreaElement,
  edit: TextEdit,
  fallback: (value: string) => void,
) {
  // A collapsed range and nothing to insert is a no-op — and `delete` on a
  // collapsed selection would eat the character before it.
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
 * A markdown note, edited in place. No toolbar: the page is the document,
 * with a title above the text and the mode and save state in the host's
 * header (`DocumentControls`).
 *
 * A picture can be pasted or dropped into it, and unlike a composer's it is
 * written the moment it arrives — see `embed` below.
 *
 * The draft lives in a ref as well as in state, and every write reads the
 * ref: a save that starts on a timer, a blur, ⌘S or the unmount always takes
 * the latest text, never the one a stale closure saw. Writes are serialised —
 * one in flight, looping until the draft it wrote is the draft there is — so
 * two saves can never race each other onto disk out of order.
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
  /** Why a picture could not be taken in — a refusal from Rust, or a drop of
   *  something that is not one. Its own line rather than the header's save
   *  word, which the next keystroke would overwrite. */
  const [attachError, setAttachError] = useState<string | null>(null);

  const draft = useRef("");
  const saved = useRef("");
  const timer = useRef<number | null>(null);
  const inFlight = useRef<Promise<void> | null>(null);
  const bodyRef = useRef<HTMLTextAreaElement>(null);
  const titleRef = useRef<HTMLInputElement>(null);
  /** The whole page: the drop target, so a picture can be let go anywhere
   *  over the note rather than on the text's own box, which is only as tall
   *  as what has been written. */
  const pageRef = useRef<HTMLDivElement>(null);

  // Updated after each commit rather than during render, so the cleanup that
  // runs when this editor moves to another file still sees the file it was
  // editing — React runs the old effect's cleanup before the new effects.
  const fileRef = useRef(file);
  useEffect(() => {
    fileRef.current = file;
  });
  const statusRef = useRef(onStatus);
  statusRef.current = onStatus;

  /** Write the draft if it differs from what is on disk; resolves once it
   *  does not. Safe to call from anywhere, any number of times. */
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
          // Not retried here: the next keystroke schedules another attempt,
          // and the word in the header says what happened until then.
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

  // Load the text, keyed on the row and not its path: a rename moves the
  // file under the editor and must not reload (and so discard) the draft.
  useEffect(() => {
    let live = true;
    draft.current = "";
    saved.current = "";
    setText(null);
    setLoadError(null);
    statusRef.current({ state: "idle" });
    invoke<string>("read_course_file", { relativePath: file.relative_path })
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

  // Leaving — another file, another route, the tab closing — writes whatever
  // has not been written. A save already in flight is looping on the draft
  // and will take it; only a quiet editor needs the push.
  useEffect(() => {
    return () => {
      if (timer.current != null) window.clearTimeout(timer.current);
      timer.current = null;
      if (!inFlight.current && draft.current !== saved.current) {
        saveDocument(fileRef.current, draft.current).catch(console.error);
      }
    };
  }, [file.id]);

  // The row's name follows a rename a beat later; take it unless the student
  // is mid-edit in the field.
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

  // Preview is a look at the draft, but it is also a pause — and a natural
  // moment to write. Coming back lands in the text.
  const prevMode = useRef(mode);
  useEffect(() => {
    if (mode === "preview") void flush();
    else if (prevMode.current === "preview") bodyRef.current?.focus();
    prevMode.current = mode;
  }, [mode, flush]);

  // ⌘S and ⌘⇧P, for the tab being looked at. Document-level, because in
  // preview nothing in the editor holds focus.
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
      // The row reloads through the event a moment later; until it does, a
      // save must not land on the old path.
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
   * A picture, written beside the note the moment it arrives and linked at
   * the caret.
   *
   * A composer writes nothing until send, because the message may never be
   * sent. A note has no send: the picture has to be *in* the text while it is
   * still being written around, so there is no later moment to defer the
   * write to. The cost is a file left in `assets/` when its tag is deleted
   * again, and that is the accepted trade — an image pointing at nothing
   * would be the alternative.
   *
   * The insertion goes through `applyTextEdit` like Tab and Enter do, so ⌘Z
   * takes the picture back out the way it takes back typing.
   */
  const embed = useCallback(
    async (write: (note: DbFile) => Promise<string>, name: string) => {
      try {
        const path = await write(fileRef.current);
        const ta = bodyRef.current;
        if (!ta) return;
        // `[`, `]` and `(` would close the alt text or the link early. The
        // path never needs escaping: Rust names the file itself, from a stamp
        // and the extension it sniffed.
        const alt = name.replace(/[[\]()]/g, "").trim() || "image";
        // A drop may land with the focus anywhere; the caret the edit splices
        // at is this field's, so it has to be this field that has it.
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

  /** Pasted pictures, which arrive as `File`s the clipboard owns. One at a
   *  time, so two screenshots land in the order they were pasted. A paste
   *  carrying a picture *and* a text flavour of it — most web pages — is the
   *  picture, and the composer reads one the same way. */
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

  /** …and dropped ones, which arrive as paths (`useFileDrop`). A drop of
   *  something else says so rather than being ignored: silence reads as a
   *  broken drop target, and a dropped PDF is a reasonable thing to try. */
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
      <div className="h-full flex items-center justify-center gap-2 text-muted-foreground">
        <CircleNotch size={16} className="animate-spin" />
        <span className="text-sm">Loading…</span>
      </div>
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

      {/* The affordance is an overlay over the whole note, because that is
          what accepts the drop — a zone around the text alone would be a lie
          about where a picture can be let go. */}
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
