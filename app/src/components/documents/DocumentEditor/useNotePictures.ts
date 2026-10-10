import { useCallback, useState, type RefObject } from "react";
import type { EditorView } from "@codemirror/view";

import { imagePaths } from "@/lib/harness/attachments";
import { attachDocumentFile, attachDocumentImage, pickDocumentImages } from "@/lib/notes/documents";
import type { DbFile } from "@/lib/db";

import { insertImage } from "../editor/commands";

/** Pictures into the note: pasted, dropped or picked. Each is written beside
 *  the note and linked at the caret. */
export function useNotePictures(fileRef: RefObject<DbFile>, viewRef: RefObject<EditorView | null>) {
  /** Picture-attach failures; separate from the save word, which the next
   *  keystroke would overwrite. */
  const [attachError, setAttachError] = useState<string | null>(null);

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
  }, [fileRef, viewRef]);

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

  return { attachError, pastePictures, attachPaths, pickImages };
}
